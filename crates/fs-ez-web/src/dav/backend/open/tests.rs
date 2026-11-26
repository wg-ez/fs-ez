use std::os::unix::fs::{MetadataExt, symlink};
use std::path::Path;

use dav_server::fs::FsError;
use tokio::io::AsyncReadExt;

use super::open_inbox_entry;

fn identity(path: &Path) -> (u64, u64) {
    let meta = std::fs::symlink_metadata(path).expect("lstat");
    (meta.dev(), meta.ino())
}

#[tokio::test]
async fn matching_identity_opens_the_drop() {
    let dir = tempfile::tempdir().expect("tempdir");
    let drop = dir.path().join("drop.txt");
    std::fs::write(&drop, b"inside").expect("dépôt");
    let (dev, ino) = identity(&drop);

    let mut file = open_inbox_entry(&drop, dev, ino).await.expect("ouverture");
    let mut content = Vec::new();
    file.read_to_end(&mut content).await.expect("lecture");
    assert_eq!(content, b"inside");
}

#[tokio::test]
async fn symlink_swap_is_never_followed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let drop = dir.path().join("drop.txt");
    std::fs::write(&drop, b"inside").expect("dépôt");
    let (dev, ino) = identity(&drop);

    let outside = dir.path().join("outside.txt");
    std::fs::write(&outside, b"sentinel").expect("hors racine");
    let link = dir.path().join("swap.lnk");
    symlink(&outside, &link).expect("symlink");
    std::fs::rename(&link, &drop).expect("échange");

    let error = open_inbox_entry(&drop, dev, ino)
        .await
        .expect_err("symlink refusé");
    assert_eq!(error, FsError::Forbidden);
}

#[tokio::test]
async fn swapped_regular_file_is_rejected() {
    let dir = tempfile::tempdir().expect("tempdir");
    let drop = dir.path().join("drop.txt");
    std::fs::write(&drop, b"inside").expect("dépôt");
    let (dev, ino) = identity(&drop);

    let other = dir.path().join("other.txt");
    std::fs::write(&other, b"other").expect("autre");
    std::fs::rename(&other, &drop).expect("remplacement");

    let error = open_inbox_entry(&drop, dev, ino)
        .await
        .expect_err("inode différent");
    assert_eq!(error, FsError::Forbidden);
}
