use fs_ez_app::{App, AppError, EnsureOptions};
use fs_ez_core::{FileKind, FilterExpr, Sort, SortDir, SortField, TagColor};
use fs_ez_store::FileListQuery;
use tempfile::TempDir;

mod common;

use common::{create_tag, test_app, write};

fn tag_names(app: &App, file: fs_ez_core::FileId) -> Vec<(String, bool)> {
    app.file_with_tags(file)
        .expect("fichier")
        .tags
        .into_iter()
        .map(|applied| (applied.tag.name, applied.explicit))
        .collect()
}

fn all_files_query() -> FileListQuery {
    FileListQuery {
        filter: None,
        untagged: false,
        tagged_since_days: None,
        modified_since_days: None,
        text: None,
        virtual_folder: None,
        sort: Sort {
            field: SortField::Id,
            dir: SortDir::Asc,
        },
        offset: 0,
        limit: 50,
    }
}

#[test]
fn tag_path_creates_file_tags() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    let path = write(&temp, "a.txt", b"hello");

    let result = app
        .tag_path(&path, &["Projet".to_string(), "urgent".to_string()], false)
        .expect("tag");

    assert_eq!(
        result.file.path,
        std::fs::canonicalize(&path).expect("canon")
    );
    assert_eq!(result.file.size, 5);
    assert_eq!(result.file.kind, FileKind::File);
    assert!(result.file.fingerprint.is_some());
    assert_eq!(result.tags.len(), 2);
    assert!(result.tags.iter().all(|applied| applied.explicit));
    assert_eq!(
        app.find_by_path(&result.file.path)
            .expect("lookup")
            .map(|f| f.id),
        Some(result.file.id)
    );
}

#[test]
fn retagging_does_not_duplicate_rows() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    let path = write(&temp, "a.txt", b"hello");

    let first = app
        .tag_path(&path, &["projet".to_string()], false)
        .expect("premier tag");
    let second = app
        .tag_path(&path, &["Projet".to_string()], false)
        .expect("second tag");

    assert_eq!(first.file.id, second.file.id);
    assert_eq!(second.tags.len(), 1);
    assert_eq!(app.list_tags().expect("tags").len(), 1);
}

#[test]
fn duplicate_content_is_rejected_with_first_path() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    let first = write(&temp, "first.txt", b"same");
    let second = write(&temp, "second.txt", b"same");
    app.ensure_file(&first, EnsureOptions { force: false })
        .expect("premier");

    let error = app
        .ensure_file(&second, EnsureOptions { force: false })
        .expect_err("doublon");
    match error {
        AppError::Conflict(message) => {
            assert!(message.contains("first.txt"), "message: {message}");
        }
        other => panic!("erreur inattendue: {other}"),
    }
}

#[test]
fn force_registers_missing_path_without_io() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    let path = temp.path().join("ghost.txt");

    let record = app
        .ensure_file(&path, EnsureOptions { force: true })
        .expect("force");
    assert_eq!(record.kind, FileKind::File);
    assert_eq!(record.size, 0);
    assert!(record.fingerprint.is_none());
    assert!(!path.exists());

    let error = app
        .ensure_file(&path, EnsureOptions { force: false })
        .expect_err("absent");
    assert!(matches!(error, AppError::NotFound(_)));
}

#[test]
fn directories_have_no_fingerprint_and_no_conflict() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    let first = temp.path().join("one");
    let second = temp.path().join("two");
    std::fs::create_dir(&first).expect("mkdir");
    std::fs::create_dir(&second).expect("mkdir");

    let a = app
        .ensure_file(&first, EnsureOptions { force: false })
        .expect("dossier");
    let b = app
        .ensure_file(&second, EnsureOptions { force: false })
        .expect("dossier");
    assert_eq!(a.kind, FileKind::Directory);
    assert!(a.fingerprint.is_none());
    assert!(b.fingerprint.is_none());
}

#[test]
fn untag_still_works_when_file_disappeared_from_disk() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    let path = write(&temp, "f.txt", b"x");
    let tagged = app.tag_path(&path, &["a".to_string()], false).expect("tag");
    std::fs::remove_file(&path).expect("suppression externe");

    let result = app.untag_path(&path, &["a".to_string()]).expect("untag");
    assert_eq!(result.file.id, tagged.file.id);
    assert!(result.tags.is_empty());
}

#[test]
fn implicit_tag_creation_assigns_a_palette_color() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    for name in ["auto-a", "auto-b", "auto-c"] {
        let path = write(&temp, &format!("{name}.txt"), name.as_bytes());
        let tagged = app
            .tag_path(&path, &[name.to_string()], false)
            .expect("tag");
        assert!(TagColor::ALL.contains(&tagged.tags[0].tag.color));
    }
}

#[test]
fn set_tags_replaces_the_tag_set() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    for name in ["a", "c"] {
        create_tag(&app, name, TagColor::Neutral);
    }
    let path = write(&temp, "f.txt", b"x");
    let tagged = app.tag_path(&path, &["a".to_string()], false).expect("tag");
    assert_eq!(tag_names(&app, tagged.file.id).len(), 1);

    app.set_tags(
        tagged.file.id,
        &fs_ez_app::parse_tag_names(&["c".to_string()]).expect("inputs"),
    )
    .expect("set_tags");
    assert_eq!(
        tag_names(&app, tagged.file.id),
        vec![("c".to_string(), true)]
    );

    app.set_tags(tagged.file.id, &[]).expect("vide");
    assert!(tag_names(&app, tagged.file.id).is_empty());
}

#[test]
fn merge_tags_moves_files_and_deletes_source() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    create_tag(&app, "a", TagColor::Neutral);
    create_tag(&app, "b", TagColor::Neutral);
    let first = write(&temp, "one.txt", b"one");
    let second = write(&temp, "two.txt", b"two");
    app.tag_path(&first, &["a".to_string()], false)
        .expect("tag");
    app.tag_path(&second, &["a".to_string()], false)
        .expect("tag");

    let touched = app.merge_tags(&["a".to_string()], "b").expect("fusion");
    assert_eq!(touched, 2);
    let tags = app.list_tags().expect("tags");
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].tag.name, "b");
    assert_eq!(tags[0].count, 2);
}

#[test]
fn list_recent_and_untagged() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    let tagged = write(&temp, "tagged.txt", b"tagged");
    let untagged = write(&temp, "untagged.txt", b"untagged");
    let tagged_record = app
        .tag_path(&tagged, &["x".to_string()], false)
        .expect("tag")
        .file;
    let untagged_record = app
        .ensure_file(&untagged, EnsureOptions { force: false })
        .expect("fichier");

    let page = app.list_files(&all_files_query()).expect("liste");
    assert_eq!(page.items.len(), 2);

    let recent = app.recent_files(None).expect("récents");
    assert_eq!(
        recent
            .items
            .iter()
            .map(|item| item.file.id)
            .collect::<Vec<_>>(),
        vec![tagged_record.id]
    );

    let unmatched = app.untagged_files(10).expect("non taggés");
    assert_eq!(
        unmatched
            .items
            .iter()
            .map(|item| item.file.id)
            .collect::<Vec<_>>(),
        vec![untagged_record.id]
    );

    let filtered = FileListQuery {
        filter: Some(FilterExpr::Tag {
            tag: "x".to_string(),
            negated: false,
        }),
        ..all_files_query()
    };
    assert_eq!(app.list_files(&filtered).expect("filtre").items.len(), 1);

    assert!(app.untagged_files(0).is_err());
    assert!(app.untagged_files(501).is_err());
}

#[test]
fn unknown_tag_operations_are_not_found() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    assert!(matches!(
        app.rename_tag("absent", "present"),
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        app.delete_tag("absent"),
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        app.set_tag_color("absent", TagColor::Blue, None),
        Err(AppError::NotFound(_))
    ));
}
