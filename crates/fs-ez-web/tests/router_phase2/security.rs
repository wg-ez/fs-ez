use super::*;

#[tokio::test]
async fn csrf_blocks_cross_site_and_allows_same_origin() {
    let fixture = web_fixture().await;
    let router = fixture.router();

    let response = send(
        &router,
        with_site(post_form("/login", &login_body(PASSWORD)), "cross-site"),
    )
    .await;
    assert_status(&response, StatusCode::FORBIDDEN);

    let response = send(
        &router,
        with_site(post_form("/login", &login_body(PASSWORD)), "same-origin"),
    )
    .await;
    assert_status(&response, StatusCode::SEE_OTHER);

    let id = fixture.add_file("csrf.txt", b"x", &[]);
    let body = format!("ids={id}&tags=x&action=add");
    let response = fixture
        .send(with_cookie(
            with_site(post_form("/partials/files/tags", &body), "cross-site"),
            &fixture.cookie,
        ))
        .await;
    assert_status(&response, StatusCode::FORBIDDEN);
    assert!(fixture.tag_names(id).is_empty());

    let response = fixture
        .send(with_cookie(
            with_site(post_form("/partials/files/tags", &body), "same-origin"),
            &fixture.cookie,
        ))
        .await;
    assert_status(&response, StatusCode::SEE_OTHER);
    assert_eq!(fixture.tag_names(id), ["x"]);
}

#[tokio::test]
async fn http_security_headers_and_no_content_route() {
    let fixture = web_fixture().await;
    fixture.add_file("securite.txt", b"secret", &["facture"]);

    for uri in ["/".to_string(), "/partials/files".to_string()] {
        let response = fixture.get(&uri).await;
        assert_eq!(response.status(), StatusCode::OK, "{uri}");
        let headers = response.headers();
        let csp = headers
            .get(header::CONTENT_SECURITY_POLICY)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        assert!(csp.contains("script-src 'self'"), "{uri}: {csp}");
        assert!(csp.contains("frame-ancestors 'none'"), "{uri}: {csp}");
        assert_eq!(
            headers
                .get(header::X_CONTENT_TYPE_OPTIONS)
                .and_then(|value| value.to_str().ok()),
            Some("nosniff")
        );
        assert_eq!(
            headers
                .get(header::X_FRAME_OPTIONS)
                .and_then(|value| value.to_str().ok()),
            Some("DENY")
        );
        assert!(headers.get(header::CONTENT_DISPOSITION).is_none(), "{uri}");
        assert!(
            headers
                .get(header::HeaderName::from_static("accept-ranges"))
                .is_none(),
            "{uri}"
        );
        let html = body_text(response).await;
        assert_no_inline_code(&html, &uri);
        assert!(!html.contains("secret"), "{uri}");
    }

    let response = send(&fixture.router(), get("/login")).await;
    assert_status(&response, StatusCode::OK);
    let headers = response.headers();
    let csp = headers
        .get(header::CONTENT_SECURITY_POLICY)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(csp.contains("script-src 'self'"), "/login: {csp}");
    assert!(csp.contains("frame-ancestors 'none'"), "/login: {csp}");
    assert_no_inline_code(&body_text(response).await, "/login");

    let response = send(&fixture.router(), get("/static/../Cargo.toml")).await;
    assert_status(&response, StatusCode::NOT_FOUND);

    for uri in ["/media/x", "/download/x", "/preview/x"] {
        let response = fixture.get(uri).await;
        assert_status(&response, StatusCode::NOT_FOUND);
    }
}
