use super::*;

#[tokio::test]
async fn login_page_is_standalone_and_script_free() {
    let temp = TempDir::new().expect("tempdir");
    let router = build_router(state(&temp));
    let response = send(&router, get("/login")).await;
    assert_eq!(response.status(), StatusCode::OK);
    let html = body_text(response).await;
    assert!(html.contains("action=\"/login\""));
    assert!(html.contains("/static/css/app.css?v="));
    assert!(html.contains("class=\"no-js\""));
    assert!(!html.contains("<script"));
    assert!(!html.contains("style="));
}

#[tokio::test]
async fn wrong_password_renders_generic_message_and_counts() {
    let temp = TempDir::new().expect("tempdir");
    let router = build_router(state(&temp));
    let response = send(
        &router,
        post_form("/login", &login_body("mauvais-mot-de-passe")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let html = body_text(response).await;
    assert!(html.contains("Invalid credentials"));
    assert!(!html.to_ascii_lowercase().contains("unknown"));
}
