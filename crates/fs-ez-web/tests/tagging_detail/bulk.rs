//! Edition en masse des tags.

use super::*;

#[tokio::test]
async fn bulk_adds_tags_and_counts_unknown_ids_as_failures() {
    let fixture = fixture().await;
    let first = fixture.add_file("un.txt", b"1", &[]);
    let second = fixture.add_file("deux.txt", b"2", &[]);
    let third = fixture.add_file("trois.txt", b"3", &[]);
    let token = login(&fixture.router).await;

    let body = format!("ids={first}&ids={second}&ids={third}&ids=9999&tags=un,deux&action=add");
    let response = send(
        &fixture.router,
        with_cookie(with_hx(post_form("/partials/files/tags", &body)), &token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("hx-trigger")
            .and_then(|value| value.to_str().ok()),
        Some("files-changed, tags-changed")
    );
    let html = body_text(response).await;
    assert!(html.contains("3 files updated"), "{html}");
    assert!(html.contains("1 failed"), "{html}");
    assert!(
        html.contains("hx-swap-oob=\"innerHTML:#file-tags-"),
        "{html}"
    );
    assert!(html.contains("tag-pill--"), "{html}");

    for id in [first, second, third] {
        let tags = fixture.tag_names(id);
        assert!(tags.contains(&"un".to_string()), "{id}");
        assert!(tags.contains(&"deux".to_string()), "{id}");
    }
}

#[tokio::test]
async fn bulk_scope_all_updates_every_matching_file_across_pages() {
    let fixture = Fixture::new(FixtureOptions::default().page_size(2)).await;
    let first = fixture.add_file("un.txt", b"1", &[]);
    let second = fixture.add_file("deux.txt", b"2", &[]);
    let third = fixture.add_file("trois.txt", b"3", &[]);
    let token = login(&fixture.router).await;

    let body = format!(
        "ids={first}&scope=all&scope_url={}&tags=tout&action=add",
        encode("/?sort=id:asc")
    );
    let response = send(
        &fixture.router,
        with_cookie(with_hx(post_form("/partials/files/tags", &body)), &token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let html = body_text(response).await;
    assert!(html.contains("3 files updated"), "{html}");
    assert_eq!(
        html.matches("hx-swap-oob=\"innerHTML:#file-tags-").count(),
        1
    );

    for id in [first, second, third] {
        assert!(fixture.tag_names(id).contains(&"tout".to_string()), "{id}");
    }
}

#[tokio::test]
async fn bulk_scope_all_skips_excluded_ids() {
    let fixture = Fixture::new(FixtureOptions::default().page_size(2)).await;
    let first = fixture.add_file("un.txt", b"1", &[]);
    let kept = fixture.add_file("deux.txt", b"2", &[]);
    let third = fixture.add_file("trois.txt", b"3", &[]);
    let token = login(&fixture.router).await;

    let body = format!(
        "ids={first}&scope=all&exclude_ids={kept}&scope_url={}&tags=tout&action=add",
        encode("/?sort=id:asc")
    );
    let response = send(
        &fixture.router,
        with_cookie(with_hx(post_form("/partials/files/tags", &body)), &token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let html = body_text(response).await;
    assert!(html.contains("2 files updated"), "{html}");

    assert!(fixture.tag_names(first).contains(&"tout".to_string()));
    assert!(fixture.tag_names(third).contains(&"tout".to_string()));
    assert!(!fixture.tag_names(kept).contains(&"tout".to_string()));
}

#[tokio::test]
async fn bulk_without_htmx_redirects_back() {
    let fixture = fixture().await;
    let id = fixture.add_file("un.txt", b"1", &[]);
    let token = login(&fixture.router).await;

    let mut request = post_form(
        "/partials/files/tags",
        &format!("ids={id}&tags=masse&action=add"),
    );
    request.headers_mut().insert(
        header::REFERER,
        header::HeaderValue::from_static("https://example.test/?sort=name:asc"),
    );
    let response = send(&fixture.router, with_cookie(request, &token)).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response
            .headers()
            .get(header::LOCATION)
            .and_then(|value| value.to_str().ok()),
        Some("/?sort=name:asc")
    );
    assert!(fixture.tag_names(id).contains(&"masse".to_string()));
}

#[tokio::test]
async fn bulk_accepts_more_than_a_thousand_ids() {
    let fixture = fixture().await;
    let token = login(&fixture.router).await;
    let body: Vec<String> = (1_000_000..=1_001_000)
        .map(|id| format!("ids={id}"))
        .collect();
    let body = format!("{}&tags=x&action=add", body.join("&"));
    let response = send(
        &fixture.router,
        with_cookie(with_hx(post_form("/partials/files/tags", &body)), &token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let html = body_text(response).await;
    assert!(html.contains("1001 failed"), "{html}");
}

#[tokio::test]
async fn bulk_oob_ids_refresh_only_rendered_rows() {
    let fixture = fixture().await;
    let first = fixture.add_file("un.txt", b"1", &[]);
    let second = fixture.add_file("deux.txt", b"2", &[]);
    let token = login(&fixture.router).await;

    let body = format!("ids={first}&ids={second}&oob_ids={first}&tags=rendu&action=add");
    let response = send(
        &fixture.router,
        with_cookie(with_hx(post_form("/partials/files/tags", &body)), &token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let html = body_text(response).await;
    assert!(html.contains("2 files updated"), "{html}");
    assert_eq!(
        html.matches("hx-swap-oob=\"innerHTML:#file-tags-").count(),
        1
    );
    for id in [first, second] {
        assert!(fixture.tag_names(id).contains(&"rendu".to_string()), "{id}");
    }
}

#[tokio::test]
async fn bulk_remove_and_set_go_through_the_batch_path() {
    let fixture = fixture().await;
    let first = fixture.add_file("un.txt", b"1", &["ancien", "garde"]);
    let second = fixture.add_file("deux.txt", b"2", &["ancien"]);
    let token = login(&fixture.router).await;

    let body = "scope=all&scope_url=%2F&tags=ancien&action=remove";
    let response = send(
        &fixture.router,
        with_cookie(with_hx(post_form("/partials/files/tags", body)), &token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let html = body_text(response).await;
    assert!(html.contains("2 files updated"), "{html}");
    assert_eq!(fixture.tag_names(first), vec!["garde".to_string()]);
    assert!(fixture.tag_names(second).is_empty());

    let body = "scope=all&scope_url=%2F&tags=nouveau,ajout&action=set";
    let response = send(
        &fixture.router,
        with_cookie(with_hx(post_form("/partials/files/tags", body)), &token),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let html = body_text(response).await;
    assert!(html.contains("2 files updated"), "{html}");
    for id in [first, second] {
        let mut tags = fixture.tag_names(id);
        tags.sort();
        assert_eq!(tags, vec!["ajout".to_string(), "nouveau".to_string()]);
    }
}
