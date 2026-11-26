//! Tests des opérations FS réelles.

use std::os::unix::fs::symlink;
use std::os::unix::net::UnixListener;
use std::process::Command;

use tempfile::TempDir;

use super::*;
use crate::path::path_from_bytes;

fn fs() -> SystemFs {
    SystemFs
}

#[test]
fn metadata_distinguishes_file_dir_and_symlink() {
    let temp = TempDir::new().expect("tempdir");
    let file = temp.path().join("f.txt");
    let dir = temp.path().join("d");
    let link = temp.path().join("l");
    fs::write(&file, b"hello").expect("écriture");
    fs::create_dir(&dir).expect("mkdir");
    symlink(&file, &link).expect("symlink");

    let file_meta = fs().metadata(&file).expect("meta fichier");
    assert_eq!(file_meta.kind, FileKind::File);
    assert_eq!(file_meta.size, 5);
    assert!(!file_meta.is_special);

    let dir_meta = fs().metadata(&dir).expect("meta dossier");
    assert_eq!(dir_meta.kind, FileKind::Directory);
    assert_eq!(dir_meta.size, 0);

    let link_meta = fs().metadata(&link).expect("meta lien");
    assert_eq!(link_meta.kind, FileKind::Symlink);
    assert_eq!(fs().read_link(&link).expect("cible"), file);
}

#[test]
fn fifo_and_socket_are_special() {
    let temp = TempDir::new().expect("tempdir");
    let fifo = temp.path().join("pipe");
    let status = Command::new("mkfifo").arg(&fifo).status();
    if let Ok(status) = status {
        assert!(status.success(), "mkfifo a échoué");
        let meta = fs().metadata(&fifo).expect("meta fifo");
        assert!(meta.is_special);
        assert_eq!(meta.kind, FileKind::File);
    }

    let socket = temp.path().join("sock");
    let _listener = UnixListener::bind(&socket).expect("socket");
    assert!(fs().metadata(&socket).expect("meta socket").is_special);
}

#[test]
fn read_dir_checked_rejects_symlink_and_foreign_identity() {
    let temp = TempDir::new().expect("tempdir");
    let real = temp.path().join("real");
    let moved = temp.path().join("moved");
    let target = temp.path().join("target");
    fs::create_dir(&real).expect("real");
    fs::create_dir(&target).expect("target");
    fs::write(target.join("canary.txt"), b"canary").expect("canary");
    let entry = fs()
        .read_dir(temp.path())
        .expect("listing")
        .into_iter()
        .find(|entry| entry.name.as_os_str() == "real")
        .expect("real");
    fs::rename(&real, &moved).expect("rename");
    symlink(&target, &real).expect("symlink");

    assert!(fs().read_dir_checked(&real, entry.dev, entry.ino).is_err());

    let other = temp.path().join("other");
    fs::create_dir(&other).expect("other");
    let other_entry = fs()
        .read_dir(temp.path())
        .expect("listing")
        .into_iter()
        .find(|entry| entry.name.as_os_str() == "other")
        .expect("other");
    assert!(fs().read_dir_checked(&other, entry.dev, entry.ino).is_err());
    assert!(
        fs().read_dir_checked(&other, other_entry.dev, other_entry.ino)
            .is_ok()
    );
    assert!(target.join("canary.txt").exists());
}

#[test]
fn read_dir_sorts_by_raw_bytes() {
    let temp = TempDir::new().expect("tempdir");
    fs::write(temp.path().join("b"), b"").expect("écriture");
    fs::write(temp.path().join("a"), b"").expect("écriture");
    fs::write(
        path_from_bytes({
            let mut bytes = temp.path().as_os_str().as_bytes().to_vec();
            bytes.push(b'/');
            bytes.push(0xff);
            bytes
        }),
        b"",
    )
    .expect("écriture");

    let names: Vec<Vec<u8>> = fs()
        .read_dir(temp.path())
        .expect("read_dir")
        .into_iter()
        .map(|entry| entry.name.as_os_str().as_bytes().to_vec())
        .collect();
    assert_eq!(names, vec![b"a".to_vec(), b"b".to_vec(), vec![0xff]]);
}

#[test]
fn remove_file_refuses_directory() {
    let temp = TempDir::new().expect("tempdir");
    assert!(matches!(
        fs().remove_file(temp.path()),
        Err(FsError::Invalid(_))
    ));
}

#[test]
fn writable_checks_parent_mode_bits() {
    let temp = TempDir::new().expect("tempdir");
    let target = temp.path().join("future.txt");
    assert!(fs().writable(&target));

    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o500)).expect("chmod");
    assert!(!fs().writable(&target));
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).expect("chmod");
    assert!(fs().writable(&target));
}

#[test]
fn create_dir_all_works_recursively() {
    let temp = TempDir::new().expect("tempdir");
    let nested = temp.path().join("a/b/c");
    fs().create_dir_all(&nested).expect("create_dir_all");
    assert!(nested.is_dir());
    fs().create_dir_all(&nested).expect("idempotent");
}

#[test]
fn link_noreplace_claims_only_a_free_target() {
    let temp = TempDir::new().expect("tempdir");
    let source = temp.path().join("source");
    let occupied = temp.path().join("occupied");
    let target = temp.path().join("target");
    fs::write(&source, b"source").expect("écriture");
    fs::write(&occupied, b"keep").expect("écriture");

    assert!(matches!(
        fs().link_noreplace(&source, &occupied),
        Err(FsError::AlreadyExists(_))
    ));
    assert_eq!(fs::read(&occupied).expect("contenu"), b"keep");

    fs().link_noreplace(&source, &target).expect("lien");
    assert_eq!(fs::read(&target).expect("contenu"), b"source");
}

#[test]
fn copy_exclusive_creates_target_and_refuses_existing() {
    let temp = TempDir::new().expect("tempdir");
    let source = temp.path().join("source");
    let occupied = temp.path().join("occupied");
    let target = temp.path().join("target");
    fs::write(&source, b"source").expect("écriture");
    fs::write(&occupied, b"keep").expect("écriture");

    assert!(matches!(
        fs().copy_exclusive(&source, &occupied),
        Err(FsError::AlreadyExists(_))
    ));
    assert_eq!(fs::read(&occupied).expect("contenu"), b"keep");

    fs().copy_exclusive(&source, &target).expect("copie");
    assert_eq!(fs::read(&target).expect("contenu"), b"source");
}
