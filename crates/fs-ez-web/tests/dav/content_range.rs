use super::support::*;
use crate::common::*;

#[tokio::test]
async fn put_with_content_range_patches_the_alias_without_in_place_write() {
    let dav = setup_named("note.txt", b"hello").await;
    let router = dav.fixture.router();
    let uri = tag_file_uri(TAG, &dav.alias);
    let request = content_range_put(&uri, &dav.token, "XY", "2", "bytes 1-2/5");

    let response = send(&router, request).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        std::fs::read(dav.fixture.root().join("note.txt")).expect("contenu"),
        b"hXYlo"
    );
}

#[tokio::test]
async fn put_with_content_range_offset_beyond_max_upload_is_refused() {
    let dav = setup().await;
    let router = dav.fixture.router();
    let uri = tag_file_uri(TAG, &dav.alias);
    let request = content_range_put(
        &uri,
        &dav.token,
        "x",
        "1",
        "bytes 67108864-67108864/67108865",
    );

    let response = send(&router, request).await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(
        std::fs::read(dav.fixture.root().join("note.txt")).expect("contenu"),
        b"hello"
    );
    assert_eq!(
        std::fs::metadata(dav.fixture.root().join("note.txt"))
            .expect("meta")
            .len(),
        5
    );
    assert!(dav.fixture.tmp_entries().is_empty());
}

#[tokio::test]
async fn put_with_content_range_total_beyond_max_upload_is_refused() {
    let dav = setup().await;
    let router = dav.fixture.router();
    let uri = tag_file_uri(TAG, &dav.alias);
    let request = content_range_put(&uri, &dav.token, "XY", "2", "bytes 1-2/2048");

    let response = send(&router, request).await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(
        std::fs::read(dav.fixture.root().join("note.txt")).expect("contenu"),
        b"hello"
    );
    assert!(dav.fixture.tmp_entries().is_empty());
}

#[tokio::test]
async fn put_partial_refuses_existing_target_beyond_max_upload() {
    let content = vec![b'a'; 2048];
    let dav = setup_named("note.txt", &content).await;
    let router = dav.fixture.router();
    let uri = tag_file_uri(TAG, &dav.alias);
    let request = content_range_put(&uri, &dav.token, "b", "1", "bytes 0-0/*");

    let response = send(&router, request).await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(
        std::fs::read(dav.fixture.root().join("note.txt")).expect("contenu"),
        content
    );
    assert!(dav.fixture.tmp_entries().is_empty());
}

#[tokio::test]
async fn put_partial_at_max_upload_boundary_succeeds() {
    let content = vec![b'a'; 1024];
    let dav = setup_named("note.txt", &content).await;
    let router = dav.fixture.router();
    let uri = tag_file_uri(TAG, &dav.alias);
    let request = content_range_put(&uri, &dav.token, "b", "1", "bytes 0-0/1024");

    let response = send(&router, request).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let mut expected = content;
    expected[0] = b'b';
    assert_eq!(
        std::fs::read(dav.fixture.root().join("note.txt")).expect("contenu"),
        expected
    );
}
