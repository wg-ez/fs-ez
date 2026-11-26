use super::*;

#[tokio::test]
async fn tags_without_files_are_hidden_and_not_resolvable() {
    let dav = setup_named("note.txt", b"hello").await;
    dav.fixture.create_tag("Empty", TagColor::Red);
    let router = dav.fixture.router();

    let body =
        assert_multi_status(send(&router, propfind("/dav/tags/", &dav.token, "1")).await).await;
    assert!(
        !body.contains("<D:href>/dav/tags/Empty/</D:href>"),
        "{body}"
    );
    assert!(
        body.contains("<D:href>/dav/tags/All%20Empty/</D:href>"),
        "{body}"
    );

    let response = send(&router, propfind("/dav/tags/Empty/", &dav.token, "1")).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let response = send(&router, propfind("/dav/tags/work/Empty/", &dav.token, "1")).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn all_empty_lists_tags_without_files_at_root_and_in_tag_folders() {
    let dav = setup_named("note.txt", b"hello").await;
    dav.fixture.create_tag("Empty", TagColor::Red);
    let router = dav.fixture.router();

    let body = assert_multi_status(
        send(&router, propfind("/dav/tags/All%20Empty/", &dav.token, "1")).await,
    )
    .await;
    assert!(
        body.contains("<D:href>/dav/tags/All%20Empty/Empty/</D:href>"),
        "{body}"
    );
    assert!(
        !body.contains("<D:href>/dav/tags/All%20Empty/work/</D:href>"),
        "{body}"
    );

    let body =
        assert_multi_status(send(&router, propfind("/dav/tags/work/", &dav.token, "1")).await)
            .await;
    assert!(
        body.contains("<D:href>/dav/tags/work/All%20Empty/</D:href>"),
        "{body}"
    );

    let body = assert_multi_status(
        send(
            &router,
            propfind("/dav/tags/work/All%20Empty/", &dav.token, "1"),
        )
        .await,
    )
    .await;
    assert!(
        body.contains("<D:href>/dav/tags/work/All%20Empty/Empty/</D:href>"),
        "{body}"
    );

    let body = assert_multi_status(
        send(
            &router,
            propfind("/dav/tags/All%20Empty/Empty/", &dav.token, "1"),
        )
        .await,
    )
    .await;
    assert!(
        body.contains("<D:href>/dav/tags/All%20Empty/Empty/All%20Files/</D:href>"),
        "{body}"
    );
}

#[tokio::test]
async fn upload_through_all_empty_tags_a_tag_on_its_first_file() {
    let dav = setup_named("note.txt", b"hello").await;
    dav.fixture.create_tag("Empty", TagColor::Red);
    let router = dav.fixture.router();
    let uri = "/dav/tags/All%20Empty/Empty/All%20Files/first.txt";

    let response = send(
        &router,
        dav_request(Method::PUT, uri, &dav.token, Body::from("first")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let id = dav.fixture.file_id("first.txt");
    assert_eq!(dav.fixture.tag_names(id), vec!["Empty".to_string()]);

    // L'URL reste lisible après le PUT, le tag n'est plus vide mais l'accès par `All Empty` le retrouve.
    let response = dav_get(&router, uri, &dav.token).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_text(response).await, "first");

    let body = assert_multi_status(
        send(&router, propfind("/dav/tags/All%20Empty/", &dav.token, "1")).await,
    )
    .await;
    assert!(
        !body.contains("<D:href>/dav/tags/All%20Empty/Empty/</D:href>"),
        "{body}"
    );

    let body =
        assert_multi_status(send(&router, propfind("/dav/tags/", &dav.token, "1")).await).await;
    assert!(body.contains("<D:href>/dav/tags/Empty/</D:href>"), "{body}");
}

#[tokio::test]
async fn direct_upload_under_an_empty_tag_imports_and_tags_the_file() {
    let dav = setup_named("note.txt", b"hello").await;
    dav.fixture.create_tag("Empty", TagColor::Red);
    let router = dav.fixture.router();
    let uri = "/dav/tags/All%20Empty/Empty/direct.txt";

    let response = send(
        &router,
        dav_request(Method::PUT, uri, &dav.token, Body::from("direct")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let id = dav.fixture.file_id("direct.txt");
    assert_eq!(dav.fixture.tag_names(id), vec!["Empty".to_string()]);

    let response = dav_get(&router, uri, &dav.token).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_text(response).await, "direct");
}
