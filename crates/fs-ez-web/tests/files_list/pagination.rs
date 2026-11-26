use super::*;

#[tokio::test]
async fn page_renders_the_first_page_and_ignores_url_limit() {
    let fixture = fixture(Some(10)).await;
    let token = login(&fixture.router).await;
    let (status, html) = page(&fixture.router, &token, "/?limit=1").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(row_count(&html), 10);
    assert!(html.contains("data-total=\"25\""), "{html}");
    assert!(!html.contains("data-block-size"), "{html}");
    assert!(!html.contains("data-virtual"), "{html}");
    assert!(!html.contains("files-sentinel"), "{html}");
    assert!(!html.contains("data-next-offset"), "{html}");
}

#[tokio::test]
async fn offset_chunks_are_contiguous_without_duplicates() {
    let fixture = fixture(Some(10)).await;
    let token = login(&fixture.router).await;
    let first = "/partials/files/rows?sort=id%3Aasc&offset=0&limit=10";
    let second = "/partials/files/rows?sort=id%3Aasc&offset=10&limit=10";
    let (status, first_html) = page(&fixture.router, &token, first).await;
    assert_eq!(status, StatusCode::OK);
    let (status, second_html) = page(&fixture.router, &token, second).await;
    assert_eq!(status, StatusCode::OK);

    let first_ids = selected_ids(&first_html);
    let second_ids = selected_ids(&second_html);
    assert_eq!(first_ids.len(), 10);
    assert_eq!(second_ids.len(), 10);
    let unique: HashSet<&String> = first_ids.iter().chain(second_ids.iter()).collect();
    assert_eq!(unique.len(), 20);
}

#[tokio::test]
async fn rows_endpoint_returns_only_rows_without_sentinel() {
    let fixture = fixture(Some(10)).await;
    let token = login(&fixture.router).await;
    let uri = "/partials/files/rows?sort=id%3Aasc&offset=0&limit=10";
    let response = send(&fixture.router, with_cookie(get(uri), &token)).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::VARY)
            .and_then(|value| value.to_str().ok()),
        Some("HX-Request")
    );
    let html = body_text(response).await;
    assert!(!html.contains("<html"));
    assert!(!html.contains("<table"));
    assert!(!html.contains("files-sentinel"));
    assert!(!html.contains("data-next-offset"));
    assert_eq!(row_count(&html), 10);
}

#[tokio::test]
async fn rows_limit_defaults_to_page_size_and_clamps_low_values() {
    let fixture = fixture(Some(10)).await;
    let token = login(&fixture.router).await;
    let (status, html) = page(
        &fixture.router,
        &token,
        "/partials/files/rows?sort=id%3Aasc&offset=0",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(row_count(&html), 10);

    let (status, html) = page(
        &fixture.router,
        &token,
        "/partials/files/rows?sort=id%3Aasc&offset=0&limit=0",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(row_count(&html), 1);
}

#[tokio::test]
async fn rows_limit_is_clamped_to_five_hundred() {
    let fixture = fixture(None).await;
    let token = login(&fixture.router).await;
    let base = OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("timestamp");
    for index in 0..500_u64 {
        let name = format!("extra-{index:03}.txt");
        let path = fixture.root().join(&name);
        fixture
            .app
            .store
            .insert_file(&NewFile {
                path: path.clone(),
                path_display: path.to_string_lossy().into_owned(),
                name,
                dir: fixture.root().to_string_lossy().into_owned(),
                size: 1,
                mtime: base,
                kind: FileKind::File,
                fingerprint: None,
            })
            .expect("fichier");
    }
    let (status, html) = page(
        &fixture.router,
        &token,
        "/partials/files/rows?sort=id%3Aasc&offset=0&limit=9999",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(row_count(&html), 500);
}

#[tokio::test]
async fn offset_jump_returns_the_expected_chunk() {
    let fixture = fixture(Some(10)).await;
    let token = login(&fixture.router).await;
    let (_, first) = page(&fixture.router, &token, "/?sort=id:asc").await;
    let uri = "/partials/files/rows?sort=id%3Aasc&offset=20&limit=10";
    let (status, block) = page(&fixture.router, &token, uri).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(row_count(&block), 5);
    let first_ids = selected_ids(&first);
    for id in selected_ids(&block) {
        assert!(!first_ids.contains(&id), "doublon {id}");
    }
}

#[tokio::test]
async fn invalid_offset_limit_and_unknown_values_are_bad_request() {
    let fixture = fixture(None).await;
    let token = login(&fixture.router).await;
    for uri in [
        "/?offset=-1",
        "/?limit=nope",
        "/?sort=date:up",
        "/?filter=bogus",
        "/?tag=a=b",
        "/partials/files/rows?offset=nope",
        "/partials/files/rows?offset=1000001",
        "/partials/files/rows?limit=nope",
    ] {
        let (status, _) = page(&fixture.router, &token, uri).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
    }
    let (status, _) = page(&fixture.router, &token, &format!("/?q={}", "a".repeat(201))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
