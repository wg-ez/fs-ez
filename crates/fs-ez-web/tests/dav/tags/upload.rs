use super::*;

#[tokio::test]
async fn put_new_file_in_tag_collection_imports_and_applies_tags() {
    let dav = setup_nested().await;
    let router = dav.fixture.router();
    let uri = "/dav/tags/work/Julien/All%20Files/uploaded.txt";

    let response = send(
        &router,
        dav_request(Method::PUT, uri, &dav.token, Body::from("uploaded")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let response = dav_get(&router, uri, &dav.token).await;
    assert_eq!(body_text(response).await, "uploaded");

    let id = dav.fixture.file_id("uploaded.txt");
    let mut tags = dav.fixture.tag_names(id);
    tags.sort();
    assert_eq!(tags, ["Julien".to_string(), "work".to_string()]);
}

#[tokio::test]
async fn put_new_file_directly_under_a_tag_selection_imports_and_applies_tags() {
    let dav = setup_nested().await;
    let router = dav.fixture.router();

    let deepest = "/dav/tags/work/Julien/Opus/deep-upload.opus";
    let response = send(
        &router,
        dav_request(Method::PUT, deepest, &dav.token, Body::from("deep")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let id = dav.fixture.file_id("deep-upload.opus");
    let mut tags = dav.fixture.tag_names(id);
    tags.sort();
    assert_eq!(
        tags,
        ["Julien".to_string(), "Opus".to_string(), "work".to_string()]
    );

    let loose = "/dav/tags/work/loose-upload.opus";
    let response = send(
        &router,
        dav_request(Method::PUT, loose, &dav.token, Body::from("loose")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let id = dav.fixture.file_id("loose-upload.opus");
    assert_eq!(dav.fixture.tag_names(id), ["work".to_string()]);

    let response = send(
        &router,
        propfind("/dav/tags/work/All%20Files/", &dav.token, "1"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::MULTI_STATUS);
    let body = body_text(response).await;
    assert!(body.contains("loose-upload.opus"), "{body}");
}

#[tokio::test]
async fn deepest_selection_accepts_direct_upload_then_move() {
    let dav = setup_nested().await;
    let router = dav.fixture.router();
    let temp = "/dav/tags/work/Julien/Opus/direct.opus";
    let destination = "/dav/tags/work/Julien/Opus/three.opus";

    let response = send(
        &router,
        dav_request(Method::PUT, temp, &dav.token, Body::from("direct")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let response = move_folder(&router, temp, destination, &dav.token).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let response = dav_get(&router, destination, &dav.token).await;
    assert_eq!(body_text(response).await, "direct");
}

#[tokio::test]
async fn nested_tag_copy_imports_files_with_selection_tags() {
    let dav = setup().await;
    let router = dav.fixture.router();
    let folder = format!("/dav/tags/{TAG}/{FILES}/album");

    let response = send(
        &router,
        dav_request(mkcol(), &folder, &dav.token, Body::empty()),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let uri = format!("{folder}/photo.pdf");
    let response = send(
        &router,
        dav_request(Method::PUT, &uri, &dav.token, Body::from("photo")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let id = dav.fixture.file_id("photo.pdf");
    assert_eq!(dav.fixture.tag_names(id), vec![TAG.to_string()]);

    let response = send(&router, propfind(&folder, &dav.token, "0")).await;
    assert_eq!(response.status(), StatusCode::MULTI_STATUS);
    let response = send(&router, propfind(&uri, &dav.token, "0")).await;
    assert_eq!(response.status(), StatusCode::MULTI_STATUS);
}
