use std::fs;
use std::os::unix::fs::{MetadataExt, symlink};

use tempfile::TempDir;

use super::*;

fn process_owner() -> FileOwner {
    let meta = fs::metadata("/proc/self").expect("processus");
    FileOwner {
        uid: meta.uid(),
        gid: meta.gid(),
    }
}

#[test]
fn walk_applies_every_entry() {
    let temp = TempDir::new().expect("tempdir");
    fs::create_dir(temp.path().join("a")).expect("dossier");
    fs::write(temp.path().join("a/b.txt"), b"b").expect("fichier");
    fs::write(temp.path().join("c.txt"), b"c").expect("fichier");
    symlink(temp.path().join("c.txt"), temp.path().join("lien")).expect("lien");
    let skip = temp.path().join("skip");
    fs::create_dir(&skip).expect("dossier");
    fs::write(skip.join("garde.txt"), b"garde").expect("fichier");

    let pinned = PinnedDir::open(temp.path()).expect("épinglé");
    let mut report = OwnerReport::default();
    walk(&pinned, process_owner(), &mut report, 0);
    assert_eq!(report.applied, 6);
    assert_eq!(report.skipped, 0);
    assert_eq!(report.failed, 0);
}

#[test]
fn state_owner_target_decides_on_pinned_inode() {
    let root = TempDir::new().expect("root");
    let root_path = fs::canonicalize(root.path()).expect("canon root");
    let state = root_path.join("config");
    fs::create_dir(&state).expect("état");
    let pinned = PinnedDir::open(&state).expect("état épinglé");
    assert!(!state_owner_target(&root_path, &pinned).expect("sous root"));

    let outside = TempDir::new().expect("hors");
    let outside_path = fs::canonicalize(outside.path()).expect("canon hors");
    let pinned = PinnedDir::open(&outside_path).expect("hors épinglé");
    assert!(state_owner_target(&root_path, &pinned).expect("hors root"));

    let link = root_path.join("lien");
    symlink(&outside_path, &link).expect("symlink");
    assert!(PinnedDir::open(&link).is_err());
}

#[test]
fn walk_keeps_pinned_inode_after_state_swap() {
    let temp = TempDir::new().expect("tempdir");
    let state = temp.path().join("state");
    fs::create_dir(&state).expect("état");
    fs::write(state.join("garde.txt"), b"garde").expect("fichier");
    let victim = TempDir::new().expect("victime");
    for name in ["v1", "v2", "v3"] {
        fs::write(victim.path().join(name), b"x").expect("canari");
    }

    let pinned = PinnedDir::open(&state).expect("épinglé");
    fs::rename(&state, temp.path().join("state.real")).expect("renommage");
    symlink(victim.path(), &state).expect("symlink");

    let mut report = OwnerReport::default();
    chown_tree(&pinned, process_owner(), &mut report);
    // Seuls le dossier épinglé et son fichier d'origine comptent, la cible substituée est ignorée.
    assert_eq!(report.applied, 2);
    assert_eq!(report.failed, 0);
}

#[test]
fn can_apply_accepts_root_or_matching_identity() {
    let owner = process_owner();
    assert!(can_apply(owner));
    let other = FileOwner {
        uid: owner.uid.wrapping_add(1),
        gid: owner.gid,
    };
    let meta = fs::metadata("/proc/self").expect("processus");
    assert_eq!(can_apply(other), meta.uid() == 0);
}
