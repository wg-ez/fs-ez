use super::*;

#[tokio::test]
async fn partial_returns_fragment_only() {
    let fixture = fixture(None).await;
    let token = login(&fixture.router).await;
    let response = send(&fixture.router, with_cookie(get("/partials/files"), &token)).await;
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
    assert!(!html.contains("<body"));
    assert!(html.contains("data-col-resizable"));
}
