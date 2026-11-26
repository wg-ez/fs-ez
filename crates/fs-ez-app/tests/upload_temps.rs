//! Tests de purge des temporaires d'upload DAV.

use std::time::{Duration, SystemTime};

mod common;

use common::{Harness, set_modified};

#[test]
fn old_upload_temps_are_purged() {
    let harness = Harness::with_files();
    let dir = harness.app.upload_tmp_dir();
    std::fs::create_dir_all(&dir).expect("tmp");
    let old = dir.join("upload-old.part");
    let fresh = dir.join("upload-fresh.part");
    let other = dir.join("note.txt");
    std::fs::write(&old, b"old").expect("écriture");
    std::fs::write(&fresh, b"fresh").expect("écriture");
    std::fs::write(&other, b"other").expect("écriture");
    let stamp = SystemTime::now() - Duration::from_secs(25 * 3600);
    set_modified(&old, stamp);

    assert_eq!(harness.app.purge_upload_temps().expect("purge"), 1);
    assert!(!old.exists());
    assert!(fresh.exists());
    assert!(other.exists());
    assert_eq!(harness.app.purge_upload_temps().expect("idempotent"), 0);
}

#[test]
fn dav_temps_are_purged_without_following_symlinks() {
    let harness = Harness::with_files();
    let dir = harness.app.upload_tmp_dir();
    let collection = dir.join("dav").join("a".repeat(64));
    std::fs::create_dir_all(&collection).expect("dav");
    let staging = dir.join("dav-label-token.part");
    let nested = collection.join("b".repeat(64));
    let fresh = dir.join("dav-fresh.part");
    let user = harness.temp.path().join("files").join("user.txt");
    let outside = harness.temp.path().join("outside.txt");
    let trapped = collection.join("c".repeat(64));
    std::fs::write(&staging, b"staging").expect("staging");
    std::fs::write(&nested, b"nested").expect("nested");
    std::fs::write(&fresh, b"fresh").expect("fresh");
    std::fs::write(&user, b"indexed").expect("user");
    std::fs::write(&outside, b"outside").expect("outside");
    std::os::unix::fs::symlink(&outside, &trapped).expect("lien");
    let old = SystemTime::now() - Duration::from_secs(25 * 3600);
    for path in [&staging, &nested, &outside] {
        set_modified(path, old);
    }

    assert_eq!(harness.app.purge_upload_temps().expect("purge"), 2);
    assert!(!staging.exists());
    assert!(!nested.exists());
    assert!(fresh.exists());
    assert!(trapped.symlink_metadata().is_ok());
    assert!(outside.exists());
    assert!(user.exists());
    assert_eq!(harness.app.purge_upload_temps().expect("idempotent"), 0);
}
