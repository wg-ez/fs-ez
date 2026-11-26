use super::*;

#[tokio::test]
async fn tag_collection_lists_files_and_unselected_tags() {
    let dav = setup_nested().await;
    let router = dav.fixture.router();

    let body =
        assert_multi_status(send(&router, propfind("/dav/tags/work/", &dav.token, "1")).await)
            .await;
    assert!(
        body.contains("<D:href>/dav/tags/work/All%20Files/</D:href>"),
        "{body}"
    );
    assert!(
        body.contains("<D:href>/dav/tags/work/Julien/</D:href>"),
        "{body}"
    );
    assert!(
        body.contains("<D:href>/dav/tags/work/Opus/</D:href>"),
        "{body}"
    );
    assert!(
        !body.contains("<D:href>/dav/tags/work/Solo/</D:href>"),
        "{body}"
    );
    assert!(!body.contains("three.opus"), "{body}");

    let body = assert_multi_status(
        send(&router, propfind("/dav/tags/work/Julien/", &dav.token, "1")).await,
    )
    .await;
    assert!(
        body.contains("<D:href>/dav/tags/work/Julien/All%20Files/</D:href>"),
        "{body}"
    );
    assert!(
        body.contains("<D:href>/dav/tags/work/Julien/Opus/</D:href>"),
        "{body}"
    );
    assert!(
        !body.contains("<D:href>/dav/tags/work/Julien/work/"),
        "{body}"
    );

    let body = assert_multi_status(
        send(
            &router,
            propfind("/dav/tags/work/Julien/Opus/", &dav.token, "1"),
        )
        .await,
    )
    .await;
    assert!(
        body.contains("<D:href>/dav/tags/work/Julien/Opus/three.opus</D:href>"),
        "{body}"
    );
    assert!(
        !body.contains("<D:href>/dav/tags/work/Julien/Opus/All%20Files/</D:href>"),
        "{body}"
    );
    assert!(
        !body.contains("<D:href>/dav/tags/work/Julien/Opus/work/"),
        "{body}"
    );
    assert!(
        !body.contains("<D:href>/dav/tags/work/Julien/Opus/Julien/"),
        "{body}"
    );
}

#[tokio::test]
async fn files_folder_contains_every_superset_of_the_selected_tags() {
    let dav = setup_nested().await;
    let router = dav.fixture.router();

    let body = assert_multi_status(
        send(
            &router,
            propfind("/dav/tags/work/All%20Files/", &dav.token, "1"),
        )
        .await,
    )
    .await;
    assert!(body.contains("three.opus"), "{body}");
    assert!(body.contains("two.opus"), "{body}");
    assert!(!body.contains("other.opus"), "{body}");

    let body = assert_multi_status(
        send(
            &router,
            propfind("/dav/tags/work/Julien/All%20Files/", &dav.token, "1"),
        )
        .await,
    )
    .await;
    assert!(body.contains("three.opus"), "{body}");
    assert!(body.contains("two.opus"), "{body}");
    assert!(!body.contains("other.opus"), "{body}");

    let body = assert_multi_status(
        send(
            &router,
            propfind("/dav/tags/work/Julien/Opus/All%20Files/", &dav.token, "1"),
        )
        .await,
    )
    .await;
    assert!(body.contains("three.opus"), "{body}");
    assert!(!body.contains("two.opus"), "{body}");
    assert!(!body.contains("other.opus"), "{body}");
}

#[tokio::test]
async fn nested_files_folder_reads_members_and_hides_non_members() {
    let dav = setup_nested().await;
    let router = dav.fixture.router();

    let response = dav_get(&router, "/dav/tags/work/Julien/Opus/three.opus", &dav.token).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_text(response).await, "three");

    let response = dav_get(&router, "/dav/tags/work/Julien/Opus/two.opus", &dav.token).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let response = dav_get(
        &router,
        "/dav/tags/work/Julien/All%20Files/two.opus",
        &dav.token,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_text(response).await, "two");

    let response = dav_get(
        &router,
        "/dav/tags/work/Julien/Opus/All%20Files/two.opus",
        &dav.token,
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let response = dav_get(
        &router,
        "/dav/tags/work/Opus/Julien/All%20Files/three.opus",
        &dav.token,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let response = dav_get(
        &router,
        "/dav/tags/work/work/All%20Files/three.opus",
        &dav.token,
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn tag_named_all_files_does_not_shadow_the_files_folder() {
    let dav = setup_named("note.txt", b"hello").await;
    let tag = dav.fixture.create_tag("All Files", TagColor::Red);
    dav.fixture
        .app
        .tag_path(
            &dav.fixture.root().join("note.txt"),
            &["All Files".to_string()],
            false,
        )
        .expect("tag all files");
    let router = dav.fixture.router();

    let body =
        assert_multi_status(send(&router, propfind("/dav/tags/", &dav.token, "1")).await).await;
    assert!(
        body.contains(&format!(
            "<D:href>/dav/tags/All%20Files-{}/</D:href>",
            tag.id.0
        )),
        "{body}"
    );

    let body = assert_multi_status(
        send(
            &router,
            propfind(
                &format!("/dav/tags/All%20Files-{}/", tag.id.0),
                &dav.token,
                "1",
            ),
        )
        .await,
    )
    .await;
    assert!(
        body.contains(&format!(
            "<D:href>/dav/tags/All%20Files-{}/All%20Files/</D:href>",
            tag.id.0
        )),
        "{body}"
    );
}

#[tokio::test]
async fn propfind_depth_one_lists_each_file_with_its_tags_and_properties() {
    let dav = setup_nested().await;
    let router = dav.fixture.router();

    let body = assert_multi_status(
        send(
            &router,
            propfind("/dav/tags/work/Julien/All%20Files/", &dav.token, "1"),
        )
        .await,
    )
    .await;
    assert!(body.contains("three.opus"), "{body}");
    assert!(body.contains("two.opus"), "{body}");
    assert!(!body.contains("other.opus"), "{body}");
    assert!(!body.contains("alone.opus"), "{body}");
    assert!(body.matches("<D:getetag>").count() >= 2, "{body}");
    assert!(body.matches("<D:getlastmodified>").count() >= 2, "{body}");
}
