//! Dossiers virtuels : import structuré, navigation et déplacements.

mod common;

use common::{Harness, ScriptedFs, app_with_fs, write};
use fs_ez_app::virtual_folders::MAX_VIRTUAL_FOLDER_DEPTH;
use fs_ez_app::{AppError, EnsureOptions, FileListRequest};
use fs_ez_core::{VirtualFolder, VirtualFolderId};

fn write_inbox(harness: &Harness, relative: &str, content: &[u8]) {
    let path = harness.temp.path().join("inbox").join(relative);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("dossier inbox");
    std::fs::write(path, content).expect("fichier inbox");
}

fn folder(app: &Harness, parent: Option<VirtualFolderId>, name: &str) -> VirtualFolder {
    app.app
        .virtual_folder_children(parent)
        .expect("dossiers virtuels")
        .into_iter()
        .find(|folder| folder.name == name)
        .unwrap_or_else(|| panic!("dossier {name} absent"))
}

fn create(harness: &Harness, parent: Option<VirtualFolderId>, name: &str) -> VirtualFolder {
    harness
        .app
        .create_virtual_folder(parent, name)
        .expect("dossier créé")
}

fn ensure(harness: &Harness, relative: &str, content: &[u8]) -> fs_ez_core::FileRecord {
    harness
        .app
        .ensure_file(
            &harness.write(relative, content),
            EnsureOptions { force: false },
        )
        .expect("fichier")
}

fn inbox_date() -> String {
    fs_ez_core::datetime::now_local().date().to_string()
}

fn dated_inbox(harness: &Harness) -> VirtualFolder {
    let root = harness.app.virtual_folder_children(None).expect("racine");
    assert_eq!(root.len(), 1);
    assert_eq!(root[0].name, "inbox");
    folder(harness, Some(root[0].id), &inbox_date())
}

fn parent_of(harness: &Harness, id: VirtualFolderId) -> Option<VirtualFolderId> {
    harness.app.get_virtual_folder(id).expect("dossier").parent
}

fn files_in(harness: &Harness, folder: VirtualFolderId) -> Vec<String> {
    harness
        .app
        .files_in_virtual_folder(Some(folder), 0, 50)
        .expect("fichiers")
        .items
        .into_iter()
        .map(|item| item.file.name)
        .collect()
}

#[test]
fn inbox_import_preserves_folder_hierarchy() {
    let harness = Harness::new();
    write_inbox(&harness, "Photos/2024/a.jpg", b"a");
    write_inbox(&harness, "Photos/b.jpg", b"b");
    write_inbox(&harness, "racine.txt", b"r");

    let counts = harness.app.scan_inbox().expect("import");
    assert_eq!(counts.imported, 3);

    let date = dated_inbox(&harness);
    let photos = folder(&harness, Some(date.id), "Photos");
    let year = folder(&harness, Some(photos.id), "2024");

    assert_eq!(files_in(&harness, year.id), ["a.jpg"]);
    assert_eq!(files_in(&harness, date.id), ["racine.txt"]);

    let root_page = harness
        .app
        .files_in_virtual_folder(None, 0, 50)
        .expect("racine");
    assert!(root_page.items.is_empty());
}

#[test]
fn inbox_import_creates_empty_folders() {
    let harness = Harness::new();
    std::fs::create_dir_all(harness.temp.path().join("inbox/Projet/2024")).expect("dossier vide");
    std::fs::create_dir_all(harness.temp.path().join("inbox/.cache")).expect("caché");

    let counts = harness.app.scan_inbox().expect("import");
    assert_eq!(counts.imported, 0);

    let date = dated_inbox(&harness);
    let project = folder(&harness, Some(date.id), "Projet");
    let year = folder(&harness, Some(project.id), "2024");
    assert!(files_in(&harness, year.id).is_empty());
}

#[test]
fn inbox_imports_group_under_one_local_date() {
    let harness = Harness::new();
    write_inbox(&harness, "premier.txt", b"1");
    harness.app.scan_inbox().expect("premier import");
    write_inbox(&harness, "second.txt", b"2");
    harness.app.scan_inbox().expect("second import");

    let root = harness.app.virtual_folder_children(None).expect("racine");
    assert_eq!(root.len(), 1);
    let dates = harness
        .app
        .virtual_folder_children(Some(root[0].id))
        .expect("dates");
    assert_eq!(dates.len(), 1);
    assert_eq!(dates[0].name, inbox_date());
    let mut names = files_in(&harness, dates[0].id);
    names.sort();
    assert_eq!(names, ["premier.txt", "second.txt"]);
}

#[test]
fn deleting_a_folder_tree_deletes_descendant_files() {
    let harness = Harness::new();
    let parent = create(&harness, None, "Parent");
    let child = create(&harness, Some(parent.id), "Enfant");
    let other = create(&harness, None, "Autre");
    let file = ensure(&harness, "files/x.txt", b"x");

    harness
        .app
        .move_files_to_virtual_folder(&[file.id], Some(child.id))
        .expect("déplacement");
    assert_eq!(
        harness.app.store.folder_of_file(file.id).expect("dossier"),
        Some(child.id)
    );

    harness
        .app
        .move_virtual_folder(child.id, Some(other.id))
        .expect("déplacement dossier");
    assert_eq!(parent_of(&harness, child.id), Some(other.id));

    let report = harness
        .app
        .delete_virtual_folder_tree(other.id)
        .expect("suppression");
    assert_eq!(report.deleted, 1);
    assert_eq!(report.failed, 0);
    assert!(
        harness
            .app
            .store
            .get_file(file.id)
            .expect("fichier")
            .is_none(),
        "le fichier du sous-arbre est supprimé avec le dossier"
    );
    assert!(!harness.temp.path().join("files/x.txt").exists());
    assert!(
        harness
            .app
            .store
            .get_virtual_folder(other.id)
            .expect("dossier")
            .is_none()
    );
    assert!(
        harness
            .app
            .store
            .get_virtual_folder(child.id)
            .expect("enfant")
            .is_none()
    );
}

#[test]
fn delete_selection_ignores_child_already_removed_with_parent() {
    let harness = Harness::new();
    let parent = create(&harness, None, "Parent");
    let child = create(&harness, Some(parent.id), "Enfant");
    let file = ensure(&harness, "files/selection.txt", b"s");
    harness
        .app
        .move_files_to_virtual_folder(&[file.id], Some(child.id))
        .expect("déplacement");

    let report = harness
        .app
        .delete_selection(&[], &[parent.id, child.id])
        .expect("suppression groupée");
    assert_eq!(report.deleted, 1);
    assert_eq!(report.failed, 0);
    assert!(
        harness
            .app
            .store
            .get_virtual_folder(parent.id)
            .expect("dossier")
            .is_none()
    );
    assert!(
        harness
            .app
            .store
            .get_virtual_folder(child.id)
            .expect("enfant")
            .is_none()
    );
    assert!(
        harness
            .app
            .store
            .get_file(file.id)
            .expect("fichier")
            .is_none()
    );
}

#[test]
fn failed_unlink_is_reported_and_leaves_no_orphan_folder_link() {
    let temp = tempfile::TempDir::new().expect("tempdir");
    let fs = std::sync::Arc::new(ScriptedFs::new());
    fs.fail_remove_named("stuck.txt");
    let app = app_with_fs(&temp, fs);
    let path = write(&temp, "files/stuck.txt", b"s");
    let file = app
        .ensure_file(&path, EnsureOptions { force: false })
        .expect("fichier");
    let folder = app.create_virtual_folder(None, "Garde").expect("dossier");
    app.move_files_to_virtual_folder(&[file.id], Some(folder.id))
        .expect("déplacement");

    let report = app
        .delete_virtual_folder_tree(folder.id)
        .expect("suppression");
    assert_eq!(report.deleted, 0);
    assert_eq!(report.failed, 1);
    assert!(
        app.store
            .get_virtual_folder(folder.id)
            .expect("dossier")
            .is_none()
    );
    assert!(
        app.store.get_file(file.id).expect("fichier").is_some(),
        "l'unlink en échec laisse le fichier en bibliothèque"
    );
    assert_eq!(
        app.store.folder_of_file(file.id).expect("dossier"),
        None,
        "la cascade détache le fichier du dossier supprimé"
    );
}

#[test]
fn folder_move_refuses_cycles() {
    let harness = Harness::new();
    let parent = create(&harness, None, "Parent");
    let child = create(&harness, Some(parent.id), "Enfant");
    assert!(
        harness
            .app
            .move_virtual_folder(parent.id, Some(child.id))
            .is_err()
    );
}

#[test]
fn mixed_selection_moves_only_after_full_validation() {
    let harness = Harness::new();
    let parent = create(&harness, None, "Parent");
    let child = create(&harness, Some(parent.id), "Enfant");
    let other = create(&harness, None, "Autre");
    let file = ensure(&harness, "files/z.txt", b"z");

    let moved = harness
        .app
        .move_selection_to_virtual_folder(&[file.id], &[other.id], Some(child.id))
        .expect("déplacement mixte");
    assert_eq!(moved, 2);
    assert_eq!(
        harness.app.store.folder_of_file(file.id).expect("dossier"),
        Some(child.id)
    );
    assert_eq!(parent_of(&harness, other.id), Some(child.id));

    let error = harness
        .app
        .move_selection_to_virtual_folder(&[], &[other.id, parent.id], Some(child.id))
        .expect_err("cycle refusé");
    assert!(error.to_string().contains("subtree"), "{error}");
    assert_eq!(
        parent_of(&harness, other.id),
        Some(child.id),
        "le dossier valide reste en place quand un autre échoue"
    );
}

#[test]
fn virtual_folder_path_too_deep_is_rejected_without_partial_creation() {
    let harness = Harness::new();
    let too_deep: Vec<String> = (0..=MAX_VIRTUAL_FOLDER_DEPTH)
        .map(|index| format!("n{index}"))
        .collect();

    let error = harness
        .app
        .ensure_virtual_folder_path(&too_deep)
        .expect_err("refus au-delà de la profondeur maximale");
    assert!(matches!(error, AppError::Invalid(_)), "{error}");
    assert!(
        harness
            .app
            .virtual_folder_children(None)
            .expect("racine")
            .is_empty(),
        "aucun dossier partiel n'est créé avant le refus"
    );

    let accepted = harness
        .app
        .ensure_virtual_folder_path(&too_deep[..MAX_VIRTUAL_FOLDER_DEPTH])
        .expect("profondeur maximale acceptée");
    assert!(accepted.is_some());
}

#[test]
fn list_request_filters_on_virtual_folder() {
    let harness = Harness::new();
    let folder = create(&harness, None, "Dossier");
    let file = ensure(&harness, "files/y.txt", b"y");
    harness
        .app
        .move_files_to_virtual_folder(&[file.id], Some(folder.id))
        .expect("déplacement");

    let page = harness
        .app
        .list_files_page(&FileListRequest {
            virtual_folder: Some(fs_ez_core::VirtualFolderScope::Folder(folder.id)),
            ..FileListRequest::new(50)
        })
        .expect("liste");
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].file.id, file.id);
}

#[test]
fn virtual_folder_children_page_returns_a_window_and_the_total() {
    let harness = Harness::new();
    let root = create(&harness, None, "Racine");
    for name in ["A", "B", "C"] {
        create(&harness, Some(root.id), name);
    }

    let (window, total) = harness
        .app
        .virtual_folder_children_page(Some(root.id), 1, 1)
        .expect("fenêtre");
    assert_eq!(total, 3);
    assert_eq!(window.len(), 1);
    assert_eq!(window[0].name, "B");
}

#[test]
fn imported_virtual_folder_names_drop_invisible_formats() {
    let harness = Harness::new();
    let path = vec!["admin\u{E0041}".to_string(), "projet\u{3164}".to_string()];
    let id = harness
        .app
        .ensure_virtual_folder_path(&path)
        .expect("chemin importé")
        .expect("dossier");
    let child = harness.app.get_virtual_folder(id).expect("enfant");
    assert_eq!(child.name, "projet?");
    assert!(!child.name.contains('\u{3164}'));
    let parent = harness
        .app
        .get_virtual_folder(child.parent.expect("parent"))
        .expect("parent");
    assert_eq!(parent.name, "admin?");
    assert!(!parent.name.contains('\u{E0041}'));
}
