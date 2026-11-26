use std::os::unix::fs::symlink;
use std::path::Path;
use std::sync::Arc;

use fs_ez_fingerprint::Blake3Fingerprinter;
use fs_ez_fs::{Root, SystemFs};
use fs_ez_sqlite::{SqliteStore, db};
use fs_ez_store::Store;
use tempfile::TempDir;

use super::*;

fn test_app(temp: &TempDir) -> App {
    let store = Arc::new(SqliteStore::from_connection(
        db::open_in_memory().expect("base mémoire"),
    ));
    App::new(
        store as Arc<dyn Store>,
        Arc::new(SystemFs),
        Arc::new(Blake3Fingerprinter::default()),
        Root::discover(Some(temp.path())).expect("racine"),
    )
}

fn inbox_root(app: &App, inbox: &Path) -> InboxRoot {
    InboxRoot::discover(app.fs.as_ref(), inbox).expect("racine inbox")
}

#[test]
fn remove_inbox_source_refuses_symlinked_parent() {
    let temp = TempDir::new().expect("racine");
    let inbox = temp.path().join("inbox");
    let target = temp.path().join("cible");
    std::fs::create_dir_all(&inbox).expect("inbox");
    std::fs::create_dir_all(&target).expect("cible");
    let canary = target.join("canary.txt");
    std::fs::write(&canary, b"canary").expect("canary");
    symlink(&target, inbox.join("lien")).expect("symlink");
    let app = test_app(&temp);
    let root = inbox_root(&app, &inbox);

    app.remove_inbox_source(&root, &inbox.join("lien/canary.txt"));

    assert!(canary.exists(), "cible d'un lien jamais supprimée");
    assert_eq!(std::fs::read(&canary).expect("canary"), b"canary");
}

#[test]
fn remove_inbox_source_removes_real_inbox_file() {
    let temp = TempDir::new().expect("racine");
    let inbox = temp.path().join("inbox");
    let nested = inbox.join("sous");
    std::fs::create_dir_all(&nested).expect("sous-dossier");
    let source = nested.join("reel.txt");
    std::fs::write(&source, b"reel").expect("fichier");
    let app = test_app(&temp);
    let root = inbox_root(&app, &inbox);

    app.remove_inbox_source(&root, &source);

    assert!(!source.exists(), "source réelle retirée");
    assert!(nested.exists(), "dossier réel conservé");
}

#[test]
fn pinned_parent_unlinks_its_directory_after_swap() {
    let temp = TempDir::new().expect("racine");
    let inbox = temp.path().join("inbox");
    let nested = inbox.join("zzz-sub");
    let target = temp.path().join("cible");
    std::fs::create_dir_all(&nested).expect("sous-dossier");
    std::fs::create_dir_all(&target).expect("cible");
    let source = nested.join("victime.txt");
    std::fs::write(&source, b"interne").expect("source");
    std::fs::write(target.join("victime.txt"), b"canary").expect("canari");
    let app = test_app(&temp);
    let root = inbox_root(&app, &inbox);

    // Vérification du parent puis échange du dossier contre un lien avant l'opération.
    let (parent, name) = root.pin_parent(&source).expect("parent épinglé");
    let moved = inbox.join("zzz-sub-deplace");
    std::fs::rename(&nested, &moved).expect("rename attaquant");
    symlink(&target, &nested).expect("symlink attaquant");

    parent.remove_file(&name).expect("unlink épinglé");
    assert!(
        !moved.join("victime.txt").exists(),
        "entrée réelle retirée dans le dossier épinglé"
    );
    assert_eq!(
        std::fs::read(target.join("victime.txt")).expect("canari"),
        b"canary",
        "cible hors inbox jamais supprimée"
    );
}

#[test]
fn pinned_dir_lists_its_inode_after_swap() {
    let temp = TempDir::new().expect("racine");
    let inbox = temp.path().join("inbox");
    let nested = inbox.join("zzz-sub");
    let inner = nested.join("sous");
    let target = temp.path().join("cible");
    std::fs::create_dir_all(&inner).expect("sous-dossier");
    std::fs::create_dir_all(target.join("sous")).expect("cible");
    std::fs::write(inner.join("reel.txt"), b"interne").expect("réel");
    std::fs::write(target.join("sous/canari.txt"), b"canary").expect("canari");
    let app = test_app(&temp);
    let root = inbox_root(&app, &inbox);

    let (parent, _) = root
        .pin_parent(&inner.join("reel.txt"))
        .expect("parent épinglé");
    let moved = inbox.join("zzz-sub-deplace");
    std::fs::rename(&nested, &moved).expect("rename attaquant");
    symlink(&target, &nested).expect("symlink attaquant");

    let names: Vec<String> = parent
        .read_dir()
        .expect("liste épinglée")
        .into_iter()
        .map(|entry| entry.name.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        names,
        vec!["reel.txt".to_string()],
        "liste de l'inode épinglé"
    );
}

#[test]
fn pinned_parent_removes_only_its_directory_after_swap() {
    let temp = TempDir::new().expect("racine");
    let inbox = temp.path().join("inbox");
    let nested = inbox.join("aaa");
    let target = temp.path().join("cible");
    std::fs::create_dir_all(nested.join("bbb")).expect("sous-dossier");
    std::fs::create_dir_all(target.join("bbb")).expect("cible");
    let app = test_app(&temp);
    let root = inbox_root(&app, &inbox);

    let (parent, name) = root
        .pin_parent(&nested.join("bbb"))
        .expect("parent épinglé");
    let moved = inbox.join("aaa-deplace");
    std::fs::rename(&nested, &moved).expect("rename attaquant");
    symlink(&target, &nested).expect("symlink attaquant");

    parent.remove_dir(&name).expect("rmdir épinglé");
    assert!(!moved.join("bbb").exists(), "dossier réel vidé retiré");
    assert!(target.join("bbb").exists(), "dossier hors inbox conservé");
}
