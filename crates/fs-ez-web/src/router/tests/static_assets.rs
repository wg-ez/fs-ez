use super::*;

#[tokio::test]
async fn static_css_is_served_as_css_without_session() {
    let temp = TempDir::new().expect("tempdir");
    let router = build_router(state(&temp));
    let response = send(&router, get("/static/css/app.css")).await;
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(content_type.starts_with("text/css"), "{content_type}");
    assert_eq!(
        response
            .headers()
            .get(header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some("public, max-age=3600")
    );
}

#[tokio::test]
async fn static_traversal_is_not_found_and_head_is_supported() {
    let temp = TempDir::new().expect("tempdir");
    let router = build_router(state(&temp));
    let response = send(&router, get("/static/../Cargo.toml")).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let response = send(
        &router,
        request(Method::HEAD, "/static/css/app.css", Body::empty()),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(body_text(response).await.is_empty());
}
