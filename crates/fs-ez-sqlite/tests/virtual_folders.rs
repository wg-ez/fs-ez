//! Déplacement groupé de dossiers virtuels : atomicité et validation des cycles.

mod common;

use common::{insert, store};
use fs_ez_core::FileId;
use fs_ez_store::{StoreError, VirtualFolderStore};
use time::OffsetDateTime;

#[test]
fn move_virtual_selection_moves_folders_and_files_together() {
    let store = store();
    let root = store
        .create_virtual_folder(None, "Racine")
        .expect("dossier");
    let target = store.create_virtual_folder(None, "Cible").expect("cible");
    let file = insert(&store, "/tmp/a.txt", None, 1, OffsetDateTime::now_utc());

    let moved = store
        .move_virtual_selection(&[file], &[root.id], Some(target.id))
        .expect("déplacement groupé");
    assert_eq!(moved, 2);
    assert_eq!(
        store
            .get_virtual_folder(root.id)
            .expect("dossier")
            .unwrap()
            .parent,
        Some(target.id)
    );
    assert_eq!(
        store.folder_of_file(file).expect("dossier"),
        Some(target.id)
    );
}

#[test]
fn move_virtual_selection_rolls_back_on_invalid_file() {
    let store = store();
    let moving = store.create_virtual_folder(None, "Mobile").expect("mobile");
    let target = store.create_virtual_folder(None, "Cible").expect("cible");

    let error = store
        .move_virtual_selection(&[FileId(999_999)], &[moving.id], Some(target.id))
        .expect_err("fichier inconnu");
    assert!(
        matches!(
            error,
            StoreError::Invalid(_) | StoreError::Db(_) | StoreError::Conflict(_)
        ),
        "{error:?}"
    );
    assert_eq!(
        store
            .get_virtual_folder(moving.id)
            .expect("dossier")
            .unwrap()
            .parent,
        None,
        "le dossier ne bouge pas quand l'écriture fichier échoue"
    );
}

#[test]
fn move_virtual_selection_refuses_a_cycle() {
    let store = store();
    let parent = store.create_virtual_folder(None, "Parent").expect("parent");
    let child = store
        .create_virtual_folder(Some(parent.id), "Enfant")
        .expect("enfant");

    let error = store
        .move_virtual_selection(&[], &[parent.id], Some(child.id))
        .expect_err("cycle refusé");
    assert!(matches!(error, StoreError::Invalid(_)), "{error:?}");
    assert_eq!(
        store
            .get_virtual_folder(parent.id)
            .expect("parent")
            .unwrap()
            .parent,
        None
    );
}

#[test]
fn root_target_is_an_empty_parent() {
    let store = store();
    let folder = store
        .create_virtual_folder(None, "Dossier")
        .expect("dossier");
    store
        .move_virtual_selection(&[], &[folder.id], Some(folder.id))
        .expect_err("dossier dans lui-même");

    let child = store
        .create_virtual_folder(Some(folder.id), "Enfant")
        .expect("enfant");
    store
        .move_virtual_selection(&[], &[child.id], None)
        .expect("retour racine");
    assert_eq!(
        store
            .get_virtual_folder(child.id)
            .expect("enfant")
            .unwrap()
            .parent,
        None
    );
}
