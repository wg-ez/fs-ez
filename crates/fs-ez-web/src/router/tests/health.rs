use super::*;

#[tokio::test]
async fn health_returns_ok_and_version() {
    let temp = TempDir::new().expect("tempdir");
    let router = build_router(state(&temp));
    let response = send(&router, get("/health")).await;
    assert_eq!(response.status(), StatusCode::OK);
    let json: Value = serde_json::from_str(&body_text(response).await).expect("json");
    assert_eq!(json["status"], "ok");
    assert_eq!(json["version"], env!("CARGO_PKG_VERSION"));
}
