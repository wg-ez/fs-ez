use std::time::Duration;

use dav_server::davpath::DavPath;
use dav_server::ls::DavLockSystem;

use super::*;

fn path(name: &str) -> DavPath {
    DavPath::new(&format!("/tags/work/All Files/{name}")).expect("chemin")
}

async fn lock(ls: &ExpiringLs, path: &DavPath, timeout: Option<Duration>) -> DavLock {
    ls.lock(path, Some("fs-ez"), None, timeout, false, false)
        .await
        .expect("verrou")
}

#[tokio::test]
async fn expired_lock_stops_conflicting_and_is_reaped() {
    let ls = ExpiringLs::new();
    let path = path("note.txt");
    lock(&ls, &path, Some(Duration::from_millis(30))).await;

    assert!(ls.check(&path, None, true, false, &[]).await.is_err());
    tokio::time::sleep(Duration::from_millis(80)).await;

    assert!(ls.check(&path, None, true, false, &[]).await.is_ok());
    assert!(ls.discover(&path).await.is_empty());
    assert!(ls.inner.discover(&path).await.is_empty());
}

#[tokio::test]
async fn missing_timeout_gets_finite_expiry() {
    let ls = ExpiringLs::new();
    let path = path("note.txt");
    let lock = lock(&ls, &path, None).await;

    assert_eq!(lock.timeout, Some(DEFAULT_TIMEOUT));
    assert!(lock.timeout_at.is_some());
    assert_eq!(ls.discover(&path).await.len(), 1);
}

#[tokio::test]
async fn refresh_extends_then_expires() {
    let ls = ExpiringLs::new();
    let path = path("note.txt");
    let lock = lock(&ls, &path, Some(Duration::from_millis(30))).await;

    let _refreshed = ls
        .refresh(&path, &lock.token, Some(Duration::from_millis(300)))
        .await
        .expect("refresh");
    tokio::time::sleep(Duration::from_millis(80)).await;
    assert!(ls.check(&path, None, true, false, &[]).await.is_err());

    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(ls.check(&path, None, true, false, &[]).await.is_ok());
}

#[tokio::test]
async fn lock_count_is_capped() {
    let ls = ExpiringLs::new();
    for index in 0..MAX_LOCKS {
        let path = path(&format!("file-{index}.txt"));
        lock(&ls, &path, Some(Duration::from_secs(600))).await;
    }

    let path = path("over-cap.txt");
    let result = ls
        .lock(
            &path,
            Some("fs-ez"),
            None,
            Some(Duration::from_secs(600)),
            false,
            false,
        )
        .await;
    assert!(result.is_err());
    assert!(ls.discover(&path).await.is_empty());
}

#[test]
fn delete_covers_descendants_only() {
    let parent = DavPath::new("/tags/work/").expect("chemin");
    let child = DavPath::new("/tags/work/All Files/note.txt").expect("chemin");
    let sibling = DavPath::new("/tags/work-other/note.txt").expect("chemin");

    assert!(covers(&parent, &child));
    assert!(covers(&parent, &parent));
    assert!(!covers(&parent, &sibling));
}
