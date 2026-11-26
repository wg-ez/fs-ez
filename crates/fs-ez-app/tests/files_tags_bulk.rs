use fs_ez_app::{EnsureOptions, parse_tag_names};
use fs_ez_core::{FileId, TagColor};
use tempfile::TempDir;

mod common;

use common::{create_tag, test_app, write};

fn register(app: &fs_ez_app::App, temp: &TempDir, name: &str) -> FileId {
    let path = write(temp, name, name.as_bytes());
    app.ensure_file(&path, EnsureOptions { force: false })
        .expect("fichier")
        .id
}

fn names(app: &fs_ez_app::App, file: FileId) -> Vec<(String, bool)> {
    app.file_with_tags(file)
        .expect("fichier")
        .tags
        .into_iter()
        .map(|applied| (applied.tag.name, applied.explicit))
        .collect()
}

#[test]
fn add_bulk_dedupes_files_and_tags() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    let first = register(&app, &temp, "un.txt");
    let second = register(&app, &temp, "deux.txt");
    let inputs = parse_tag_names(&["alpha".to_string(), "alpha".to_string()]).expect("tags");

    let changed = app
        .apply_tags_bulk(&[first, second, second], &inputs, true)
        .expect("ajout");

    assert_eq!(changed, 2);
    assert_eq!(names(&app, first), vec![("alpha".to_string(), true)]);
    assert_eq!(names(&app, second), vec![("alpha".to_string(), true)]);
    assert_eq!(app.list_tags().expect("tags").len(), 1);
}

#[test]
fn remove_bulk_ignores_absent_tag() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    let first = register(&app, &temp, "un.txt");
    let second = register(&app, &temp, "deux.txt");
    app.apply_tags_bulk(&[first, second], &["alpha".to_string()], true)
        .expect("ajout");
    app.apply_tags_bulk(&[second], &["beta".to_string()], true)
        .expect("ajout beta");

    let removed = app
        .remove_tags_bulk(&[first, second], &["alpha".to_string()])
        .expect("retrait");

    assert_eq!(removed, 2);
    assert!(names(&app, first).is_empty());
    assert_eq!(names(&app, second), vec![("beta".to_string(), true)]);
}

#[test]
fn set_bulk_replaces_every_tag_set() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    for name in ["ancien", "autre", "nouveau"] {
        create_tag(&app, name, TagColor::Neutral);
    }
    let first = register(&app, &temp, "un.txt");
    let second = register(&app, &temp, "deux.txt");
    app.apply_tags_bulk(&[first, second], &["ancien".to_string()], true)
        .expect("ancien");
    app.apply_tags_bulk(&[first], &["autre".to_string()], true)
        .expect("autre");

    app.set_tags_bulk(&[first, second], &["nouveau".to_string()])
        .expect("remplacement");

    assert_eq!(names(&app, first), vec![("nouveau".to_string(), true)]);
    assert_eq!(names(&app, second), vec![("nouveau".to_string(), true)]);
}

#[test]
fn empty_selection_is_a_noop_without_creating_tags() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);

    assert_eq!(
        app.apply_tags_bulk(&[], &["fantome".to_string()], true)
            .expect("ajout vide"),
        0
    );
    assert_eq!(
        app.remove_tags_bulk(&[], &["fantome".to_string()])
            .expect("retrait vide"),
        0
    );
    assert_eq!(
        app.set_tags_bulk(&[], &["fantome".to_string()])
            .expect("remplacement vide"),
        0
    );
    assert!(app.list_tags().expect("tags").is_empty());
}

#[test]
fn missing_tag_is_created_once_then_reused() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    let first = register(&app, &temp, "un.txt");
    let second = register(&app, &temp, "deux.txt");

    app.apply_tags_bulk(&[first], &["partage".to_string()], true)
        .expect("creation");
    app.apply_tags_bulk(&[second], &["partage".to_string()], true)
        .expect("reutilisation");

    let tags = app.list_tags().expect("tags");
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].tag.name, "partage");
    assert_eq!(
        app.file_with_tags(first).expect("fichier").tags[0].tag.id,
        app.file_with_tags(second).expect("fichier").tags[0].tag.id
    );
}

#[test]
fn tag_paths_keeps_input_order_and_duplicates() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    let first = write(&temp, "un.txt", b"un");
    let second = write(&temp, "deux.txt", b"deux");
    let paths = vec![first.clone(), second.clone(), first.clone()];
    let inputs = vec!["lot".to_string()];

    let files = app.tag_paths(&paths, &inputs, false).expect("lot");

    assert_eq!(files.len(), 3);
    assert_eq!(files[0].file.id, files[2].file.id);
    assert_ne!(files[0].file.id, files[1].file.id);
    assert_eq!(files[0].file.path, first.canonicalize().expect("canon"));
    assert_eq!(files[1].file.path, second.canonicalize().expect("canon"));
    for file in &files {
        assert_eq!(file.tags.len(), 1);
        assert!(file.tags[0].explicit);
    }
    assert_eq!(app.list_tags().expect("tags").len(), 1);
}

#[test]
fn untag_paths_removes_only_requested_tag() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    let first = write(&temp, "un.txt", b"un");
    let second = write(&temp, "deux.txt", b"deux");
    let paths = vec![first.clone(), second.clone(), first.clone()];
    app.tag_paths(&paths, &["lot".to_string(), "autre".to_string()], false)
        .expect("tags");

    let files = app
        .untag_paths(&paths, &["lot".to_string()])
        .expect("retrait");

    assert_eq!(files.len(), 3);
    for file in &files {
        assert_eq!(file.tags.len(), 1);
        assert_eq!(file.tags[0].tag.name, "autre");
    }
}

#[test]
fn tag_paths_unknown_path_errors_without_force_and_creates_with_force() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    let missing = temp.path().join("absent.txt");
    let inputs = vec!["nouveau".to_string()];

    assert!(matches!(
        app.tag_paths(std::slice::from_ref(&missing), &inputs, false),
        Err(fs_ez_app::AppError::NotFound(_))
    ));
    assert!(app.list_tags().expect("tags").is_empty());

    let files = app
        .tag_paths(std::slice::from_ref(&missing), &inputs, true)
        .expect("force");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].file.size, 0);
    assert_eq!(files[0].tags.len(), 1);

    assert!(matches!(
        app.untag_paths(&[temp.path().join("inconnu.txt")], &inputs),
        Err(fs_ez_app::AppError::NotFound(_))
    ));
}
