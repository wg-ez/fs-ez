use std::ffi::OsStr;
use std::path::Path;
use std::sync::Arc;

use fs_ez_app::{App, AppError, EnsureOptions, LibraryOutcome, sharded_path};
use fs_ez_core::FileId;
use fs_ez_fs::Root;
use fs_ez_sqlite::{SqliteStore, db};
use tempfile::TempDir;

mod common;

use common::{
    Harness, ScriptedFs, app_with_fs, app_with_store, app_with_store_at_root, memory_store,
    temp_upload, test_app, write,
};

fn register(app: &App, path: &Path) -> FileId {
    app.ensure_file(path, EnsureOptions { force: false })
        .expect("fichier")
        .id
}

fn tag(app: &App, id: FileId, name: &str) {
    let inputs = fs_ez_app::parse_tag_names(&[name.to_string()]).expect("tags");
    app.apply_tags(id, &inputs, true).expect("tag");
}

#[test]
fn rename_file_moves_on_disk_and_keeps_tags() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    let source = write(&temp, "avant.txt", b"contenu");
    let id = register(&app, &source);
    tag(&app, id, "archive");

    let renamed = app.rename_file(id, "apres.txt").expect("renommage");
    let expected = std::fs::canonicalize(temp.path())
        .expect("racine")
        .join("apres.txt");

    assert_eq!(renamed.path, expected);
    assert_eq!(renamed.name, "apres.txt");
    assert_eq!(
        renamed.dir,
        expected
            .parent()
            .expect("parent")
            .to_string_lossy()
            .into_owned()
    );
    assert!(!source.exists());
    assert!(expected.exists());
    assert_eq!(app.file_with_tags(id).expect("fichier").tags.len(), 1);
}

#[test]
fn rename_file_inside_files_moves_to_the_new_name_shard() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    let files = temp.path().join("files");
    let source = sharded_path(&files, OsStr::new("avant.txt"));
    std::fs::create_dir_all(source.parent().expect("shard")).expect("shard");
    std::fs::write(&source, b"contenu").expect("écriture");
    let id = register(&app, &source);

    let renamed = app.rename_file(id, "apres.txt").expect("renommage");
    let expected = sharded_path(&files, OsStr::new("apres.txt"))
        .canonicalize()
        .expect("canon");

    assert_eq!(renamed.path, expected);
    assert_eq!(renamed.name, "apres.txt");
    assert!(!source.exists());
    assert!(expected.is_file());
}

#[test]
fn rename_file_rejects_invalid_names_and_existing_targets() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    let source = write(&temp, "a.txt", b"a");
    let other = write(&temp, "b.txt", b"b");
    let id = register(&app, &source);
    register(&app, &other);

    for invalid in ["", ".", "..", "a/b", "a\0b", "a\nb", "__proto__"] {
        assert!(
            matches!(app.rename_file(id, invalid), Err(AppError::Invalid(_))),
            "{invalid:?}"
        );
    }

    let too_long = "x".repeat(256);
    assert!(matches!(
        app.rename_file(id, &too_long),
        Err(AppError::Invalid(_))
    ));
    assert!(matches!(
        app.rename_file(id, "b.txt"),
        Err(AppError::Conflict(_))
    ));
    assert!(source.exists());
    assert!(other.exists());
}

#[test]
fn directories_cannot_be_renamed_or_deleted() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    let directory = temp.path().join("dossier");
    std::fs::create_dir(&directory).expect("dossier");
    let id = register(&app, &directory);

    assert!(matches!(
        app.rename_file(id, "autre"),
        Err(AppError::Invalid(_))
    ));
    let report = app.delete_files(&[id]).expect("suppression");
    assert_eq!(report.deleted, 0);
    assert_eq!(report.failed, 1);
    assert!(directory.is_dir());
    assert!(app.store.get_file(id).expect("lecture").is_some());
}

#[test]
fn delete_files_removes_disk_entries_and_index_rows() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    let first_path = write(&temp, "un.txt", b"1");
    let second_path = write(&temp, "deux.txt", b"2");
    let first = register(&app, &first_path);
    let second = register(&app, &second_path);
    tag(&app, first, "archive");

    let report = app
        .delete_files(&[first, second, first])
        .expect("suppression");

    assert_eq!(report.deleted, 2);
    assert_eq!(report.failed, 0);
    assert!(!first_path.exists());
    assert!(!second_path.exists());
    assert!(app.store.get_file(first).expect("lecture").is_none());
    assert!(app.store.get_file(second).expect("lecture").is_none());
}

#[test]
fn delete_files_drops_a_stale_row_for_a_missing_file() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    let path = write(&temp, "disparu.txt", b"x");
    let id = register(&app, &path);
    std::fs::remove_file(&path).expect("suppression disque");

    let report = app.delete_files(&[id]).expect("suppression");

    assert_eq!(report.deleted, 1);
    assert_eq!(report.failed, 0);
    assert!(app.store.get_file(id).expect("lecture").is_none());
}

#[test]
fn delete_files_keeps_the_row_whose_unlink_fails() {
    let temp = TempDir::new().expect("tempdir");
    let fs = Arc::new(ScriptedFs::new());
    let app = app_with_fs(&temp, fs.clone());
    let first_path = write(&temp, "un.txt", b"1");
    let second_path = write(&temp, "deux.txt", b"2");
    let first = register(&app, &first_path);
    let second = register(&app, &second_path);
    fs.fail_remove_named("deux.txt");

    let report = app.delete_files(&[first, second]).expect("suppression");

    assert_eq!(report.deleted, 1);
    assert_eq!(report.failed, 1);
    assert!(!first_path.exists());
    assert!(second_path.exists());
    assert!(app.store.get_file(first).expect("lecture").is_none());
    assert!(app.store.get_file(second).expect("lecture").is_some());
}

#[test]
fn delete_files_falls_back_to_unit_deletes_when_the_batch_rolls_back() {
    let temp = TempDir::new().expect("tempdir");
    let db_path = temp.path().join("state").join("fs-ez.db");
    std::fs::create_dir_all(db_path.parent().expect("state")).expect("state");
    let store = Arc::new(SqliteStore::open(&db_path).expect("base"));
    let app = app_with_store(&temp, store);
    let first_path = write(&temp, "un.txt", b"1");
    let second_path = write(&temp, "deux.txt", b"2");
    let first = register(&app, &first_path);
    let second = register(&app, &second_path);
    let conn = db::open(&db_path).expect("connexion");
    conn.execute(
        &format!(
            "CREATE TRIGGER refuse_delete BEFORE DELETE ON file
             WHEN OLD.id = {} BEGIN SELECT RAISE(ABORT, 'forced batch failure'); END",
            second.0
        ),
        [],
    )
    .expect("trigger");

    let report = app.delete_files(&[first, second]).expect("suppression");

    assert_eq!(report.deleted, 1);
    assert_eq!(report.failed, 1);
    assert!(!first_path.exists());
    assert!(!second_path.exists());
    assert!(app.store.get_file(first).expect("lecture").is_none());
    assert!(app.store.get_file(second).expect("lecture").is_some());
}

#[test]
fn delete_files_cascades_file_tags() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    let first_path = write(&temp, "un.txt", b"1");
    let second_path = write(&temp, "deux.txt", b"2");
    let first = register(&app, &first_path);
    let second = register(&app, &second_path);
    tag(&app, first, "archive");
    tag(&app, second, "archive");
    let before = app
        .list_tags()
        .expect("tags")
        .into_iter()
        .find(|counted| counted.tag.name == "archive")
        .expect("tag");
    assert_eq!(before.count, 2);

    app.delete_files(&[first]).expect("suppression");

    let after = app
        .list_tags()
        .expect("tags")
        .into_iter()
        .find(|counted| counted.tag.name == "archive")
        .expect("tag");
    assert_eq!(after.count, 1);
    assert!(app.store.tags_for_file(first).expect("tags").is_empty());
}

#[test]
fn content_change_keeps_the_sharded_path() {
    let harness = Harness::with_files();
    let temp = temp_upload(&harness, "upload-h.part", b"before");
    let LibraryOutcome::Indexed(file) = harness
        .app
        .import_upload(&temp, "edite.txt", None)
        .expect("import")
    else {
        panic!("import attendu");
    };
    let path = file.path.clone();
    let source = harness.app.upload_tmp_dir().join("edited-source");
    std::fs::write(&source, b"after the edit").expect("source");

    let updated = harness
        .app
        .replace_file_contents(file.id, &source, None)
        .expect("remplacement");

    assert_eq!(updated.path, path);
    assert_eq!(updated.size, 14);
    assert_eq!(std::fs::read(&path).expect("contenu"), b"after the edit");
    assert!(path.starts_with(harness.temp.path().join("files")));
}

#[test]
fn content_change_accepts_staging_outside_a_split_state_dir() {
    let root = tempfile::TempDir::new().expect("racine");
    let config = tempfile::TempDir::new().expect("config");
    std::fs::create_dir_all(root.path().join("files")).expect("files");
    let store = memory_store();
    let app = app_with_store_at_root(
        store,
        Root::discover_with(Some(root.path()), Some(config.path())).expect("root"),
    );

    let temp = app.upload_tmp_dir();
    std::fs::create_dir_all(&temp).expect("tmp");
    let upload = temp.join("upload-split.part");
    std::fs::write(&upload, b"before").expect("upload");
    let LibraryOutcome::Indexed(file) = app
        .import_upload(&upload, "edit.txt", None)
        .expect("import")
    else {
        panic!("import attendu");
    };

    let source = app.upload_tmp_dir().join("edited-source");
    std::fs::write(&source, b"after").expect("source");
    let updated = app
        .replace_file_contents(file.id, &source, None)
        .expect("remplacement");

    assert_eq!(updated.size, 5);
    assert_eq!(std::fs::read(&updated.path).expect("contenu"), b"after");
}
