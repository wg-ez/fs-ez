use fs_ez_web::state::Event;

use super::support::*;
use crate::common::*;

#[tokio::test]
async fn delete_alias_broadcasts_data_changed() {
    let dav = setup().await;
    let router = dav.fixture.router();
    let mut events = dav.fixture.state.events.subscribe();

    let response = send(
        &router,
        dav_request(
            Method::DELETE,
            &tag_file_uri(TAG, &dav.alias),
            &dav.token,
            Body::empty(),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(matches!(events.try_recv(), Ok(Event::DataChanged)));
}

#[tokio::test]
async fn delete_removes_aliases_and_temporary_uploads() {
    let dav = setup().await;
    let router = dav.fixture.router();
    let alias_uri = tag_file_uri(TAG, &dav.alias);
    let temp_uri = format!("/dav/tags/{TAG}/{FILES}/draft.docx");

    let response = send(
        &router,
        dav_request(Method::PUT, &temp_uri, &dav.token, Body::from("office")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let response = send(
        &router,
        dav_request(Method::DELETE, &temp_uri, &dav.token, Body::empty()),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let response = dav_get(&router, &temp_uri, &dav.token).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let response = send(
        &router,
        dav_request(Method::DELETE, &temp_uri, &dav.token, Body::empty()),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let response = send(
        &router,
        dav_request(
            Method::DELETE,
            &format!("/dav/tags/{TAG}/"),
            &dav.token,
            Body::empty(),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(dav.fixture.root().join("note.txt").exists());

    let response = send(
        &router,
        dav_request(Method::DELETE, &alias_uri, &dav.token, Body::empty()),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(!dav.fixture.root().join("note.txt").exists());
    assert!(
        dav.fixture
            .app
            .store
            .find_file_by_path(&dav.fixture.root().join("note.txt"))
            .expect("lecture")
            .is_none()
    );

    let response = dav_get(&router, &alias_uri, &dav.token).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn destructive_methods_and_unknown_destination_are_refused() {
    let dav = setup().await;
    let router = dav.fixture.router();
    let uri = tag_file_uri(TAG, &dav.alias);

    let response = send(
        &router,
        dav_request(mkcol(), "/dav/new", &dav.token, Body::empty()),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);

    let lock_body = "<?xml version=\"1.0\"?><D:lockinfo xmlns:D=\"DAV:\">\
        <D:lockscope><D:exclusive/></D:lockscope><D:locktype><D:write/></D:locktype>\
        <D:owner>tests</D:owner></D:lockinfo>";
    let response = send(
        &router,
        dav_request(
            Method::from_bytes(b"LOCK").expect("méthode"),
            &uri,
            &dav.token,
            Body::from(lock_body),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key("lock-token"));

    let temp_uri = format!("/dav/tags/{TAG}/{FILES}/draft.docx");
    send(
        &router,
        dav_request(Method::PUT, &temp_uri, &dav.token, Body::from("x")),
    )
    .await;
    let destination = format!("/dav/views/{VIEW_GROUP}/{VIEW}/unknown.docx");
    let response = send(
        &router,
        move_request(&temp_uri, &destination, &dav.token, None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn traversal_and_unknown_paths_are_rejected() {
    let dav = setup().await;
    let router = dav.fixture.router();

    let response = send(&router, propfind("/dav/%2e%2e/etc/passwd", &dav.token, "0")).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let response = send(
        &router,
        propfind("/dav/tags/work/%2Fetc%2Fpasswd", &dav.token, "0"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let uri = format!("/dav/tags/{TAG}/{FILES}/42-absent.txt");
    let response = dav_get(&router, &uri, &dav.token).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn put_unknown_collection_name_is_refused_and_body_limit_applies() {
    let dav = setup().await;
    let router = dav.fixture.router();

    let response = send(
        &router,
        dav_request(
            Method::PUT,
            "/dav/tags/absent/file.txt",
            &dav.token,
            Body::from("x"),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);

    let payload = vec![b'a'; 2048];
    let response = send(
        &router,
        dav_request(
            Method::PUT,
            &tag_file_uri(TAG, &dav.alias),
            &dav.token,
            Body::from(payload),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}
