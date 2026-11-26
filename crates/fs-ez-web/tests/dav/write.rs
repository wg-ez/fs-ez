use fs_ez_web::state::Event;

use super::support::*;
use crate::common::*;

#[tokio::test]
async fn put_and_move_broadcast_data_changed() {
    let dav = setup().await;
    let router = dav.fixture.router();
    let mut events = dav.fixture.state.events.subscribe();

    let uri = tag_file_uri(TAG, &dav.alias);
    let response = send(
        &router,
        dav_request(Method::PUT, &uri, &dav.token, Body::from("updated")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(matches!(events.try_recv(), Ok(Event::DataChanged)));

    let destination = format!("/dav/tags/{TAG}/{FILES}/renamed.txt");
    let response = move_folder(&router, &uri, &destination, &dav.token).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    assert!(matches!(events.try_recv(), Ok(Event::DataChanged)));
}

#[tokio::test]
async fn put_replaces_alias_atomically_and_clears_fingerprint() {
    let dav = setup().await;
    let router = dav.fixture.router();
    let uri = tag_file_uri(TAG, &dav.alias);

    let response = send(
        &router,
        dav_request(Method::PUT, &uri, &dav.token, Body::from("updated")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        std::fs::read(dav.fixture.root().join("note.txt")).expect("contenu"),
        b"updated"
    );
    let file = dav
        .fixture
        .app
        .store
        .get_file(dav.fixture.file_id("note.txt"))
        .expect("lecture")
        .expect("fichier");
    assert_eq!(file.size, 7);
    assert!(file.fingerprint.is_none());
}

#[tokio::test]
async fn move_promotes_temporary_upload_to_alias() {
    let dav = setup().await;
    let router = dav.fixture.router();
    let temp_uri = format!("/dav/tags/{TAG}/{FILES}/draft.docx");
    let response = send(
        &router,
        dav_request(Method::PUT, &temp_uri, &dav.token, Body::from("office")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let destination = tag_file_uri(TAG, &dav.alias);
    let response = move_folder(&router, &temp_uri, &destination, &dav.token).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        std::fs::read(dav.fixture.root().join("note.txt")).expect("contenu"),
        b"office"
    );

    let response = dav_get(&router, &destination, &dav.token).await;
    assert_eq!(body_text(response).await, "office");

    let temp_uri = format!("/dav/tags/{TAG}/{FILES}/draft2.docx");
    send(
        &router,
        dav_request(Method::PUT, &temp_uri, &dav.token, Body::from("absolute")),
    )
    .await;
    let absolute = format!("http://example.test/dav/tags/{TAG}/{FILES}/{}", dav.alias);
    let response = send(
        &router,
        move_request(&temp_uri, &absolute, &dav.token, Some("example.test")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        std::fs::read(dav.fixture.root().join("note.txt")).expect("contenu"),
        b"absolute"
    );
}

#[tokio::test]
async fn put_replaces_alias_with_client_mtime() {
    let dav = setup().await;
    let router = dav.fixture.router();
    let uri = tag_file_uri(TAG, &dav.alias);

    let response = send(
        &router,
        with_header(
            dav_request(Method::PUT, &uri, &dav.token, Body::from("updated")),
            "x-oc-mtime",
            "1700000000",
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_mtime(&dav.fixture.root().join("note.txt"), 1_700_000_000);
}

#[tokio::test]
async fn move_promotes_temporary_upload_with_client_mtime() {
    let dav = setup().await;
    let router = dav.fixture.router();
    let temp_uri = format!("/dav/tags/{TAG}/{FILES}/dated.docx");
    let response = send(
        &router,
        with_header(
            dav_request(Method::PUT, &temp_uri, &dav.token, Body::from("office")),
            "x-oc-mtime",
            "1700000000",
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let destination = tag_file_uri(TAG, &dav.alias);
    let response = move_folder(&router, &temp_uri, &destination, &dav.token).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_mtime(&dav.fixture.root().join("note.txt"), 1_700_000_000);
}
