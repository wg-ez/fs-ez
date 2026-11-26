use super::support::*;
use crate::common::*;

#[tokio::test]
async fn move_refuses_destination_with_another_host() {
    let dav = setup().await;
    let router = dav.fixture.router();
    let temp_uri = format!("/dav/tags/{TAG}/{FILES}/draft.docx");
    send(
        &router,
        dav_request(Method::PUT, &temp_uri, &dav.token, Body::from("office")),
    )
    .await;

    let destination = format!("http://evil.example/dav/tags/{TAG}/{FILES}/{}", dav.alias);
    let response = send(
        &router,
        move_request(&temp_uri, &destination, &dav.token, Some("example.test")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        std::fs::read(dav.fixture.root().join("note.txt")).expect("contenu"),
        b"hello"
    );
}

#[tokio::test]
async fn move_renames_alias_and_physical_file() {
    let dav = setup().await;
    let router = dav.fixture.router();
    let source = tag_file_uri(TAG, &dav.alias);
    let destination = format!("/dav/tags/{TAG}/{FILES}/renamed.txt");

    let response = send(
        &router,
        move_request(&source, &destination, &dav.token, None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    assert!(!dav.fixture.root().join("note.txt").exists());
    assert_eq!(
        std::fs::read(dav.fixture.root().join("renamed.txt")).expect("contenu"),
        b"hello"
    );

    let response = dav_get(&router, &destination, &dav.token).await;
    assert_eq!(body_text(response).await, "hello");

    let response = dav_get(&router, &source, &dav.token).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn move_rename_is_refused_across_collections() {
    let dav = setup().await;
    let router = dav.fixture.router();
    let source = tag_file_uri(TAG, &dav.alias);
    let destination = format!("/dav/views/{VIEW_GROUP}/{VIEW}/renamed.txt");

    let response = send(
        &router,
        move_request(&source, &destination, &dav.token, None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        std::fs::read(dav.fixture.root().join("note.txt")).expect("contenu"),
        b"hello"
    );
}

#[tokio::test]
async fn move_renames_alias_shared_by_multiple_tags() {
    let dav = setup().await;
    let second = dav
        .fixture
        .create_tag("second", fs_ez_core::TagColor::Amber);
    dav.fixture
        .app
        .tag_path(
            &dav.fixture.root().join("note.txt"),
            std::slice::from_ref(&second.name),
            false,
        )
        .expect("second tag");
    let router = dav.fixture.router();
    let source = tag_file_uri(TAG, &dav.alias);
    let destination = format!("/dav/tags/{TAG}/{FILES}/multi.txt");

    let response = send(
        &router,
        move_request(&source, &destination, &dav.token, None),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(
        std::fs::read(dav.fixture.root().join("multi.txt")).expect("contenu"),
        b"hello"
    );
    let id = dav.fixture.file_id("multi.txt");
    let mut tags = dav.fixture.tag_names(id);
    tags.sort();
    assert_eq!(tags, ["second".to_string(), TAG.to_string()]);

    let body = propfind_body(&router, &destination, &dav.token, "0").await;
    assert!(body.contains("multi.txt"), "{body}");
    assert!(body.contains("getetag"), "{body}");
}

#[tokio::test]
async fn move_refuses_collection_and_unknown_source() {
    let dav = setup().await;
    let router = dav.fixture.router();
    let collection = format!("/dav/tags/{TAG}/{FILES}/");
    let response = send(
        &router,
        move_request(
            &collection,
            &format!("{collection}renamed"),
            &dav.token,
            None,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let unknown = format!("/dav/tags/{TAG}/{FILES}/missing.txt");
    let response = send(
        &router,
        move_request(
            &unknown,
            &format!("/dav/tags/{TAG}/{FILES}/renamed.txt"),
            &dav.token,
            None,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
