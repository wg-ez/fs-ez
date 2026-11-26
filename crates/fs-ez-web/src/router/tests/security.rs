use super::*;

#[tokio::test]
async fn cross_site_login_is_forbidden() {
    let temp = TempDir::new().expect("tempdir");
    let router = build_router(state(&temp));
    for site in ["cross-site", "same-site"] {
        let response = send(
            &router,
            with_site(post_form("/login", &login_body(PASSWORD)), site),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{site}");
        let json: Value = serde_json::from_str(&body_text(response).await).expect("json");
        assert_eq!(json["code"], "csrf");
    }

    let response = send(
        &router,
        with_site(post_form("/login", &login_body(PASSWORD)), "same-origin"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
}

#[tokio::test]
async fn security_headers_are_present_on_every_response() {
    let temp = TempDir::new().expect("tempdir");
    let router = build_router(state(&temp));
    for uri in ["/health", "/login"] {
        let response = send(&router, get(uri)).await;
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
        assert!(headers.get(header::STRICT_TRANSPORT_SECURITY).is_some());
    }
}
