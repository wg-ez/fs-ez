use super::*;

#[tokio::test]
async fn events_stream_hello_carries_schema_version() {
    use futures_util::StreamExt;

    let temp = TempDir::new().expect("tempdir");
    let state = state(&temp);
    let router = build_router(state.clone());
    let cookie = login(&router).await;
    queue_file(&state, &temp, "evenement.txt");

    let request = json_request(Method::GET, "/api/events", None, Some(&cookie), None, None);
    let response = router.clone().oneshot(request).await.expect("réponse SSE");
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(content_type.starts_with("text/event-stream"));
    assert_eq!(
        response
            .headers()
            .get(header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some("no-cache")
    );
    assert_eq!(
        response
            .headers()
            .get("x-accel-buffering")
            .and_then(|value| value.to_str().ok()),
        Some("no")
    );

    let mut body = response.into_body().into_data_stream();
    let chunk = tokio::time::timeout(std::time::Duration::from_secs(1), body.next())
        .await
        .expect("hello sous 1 s")
        .expect("premier bloc")
        .expect("bloc valide");
    let text = String::from_utf8_lossy(&chunk);
    assert!(text.contains("event: hello"), "{text}");
    let data = text
        .lines()
        .find_map(|line| line.strip_prefix("data: "))
        .expect("ligne data");
    let data: Value = serde_json::from_str(data).expect("json hello");
    assert_eq!(data["schema"], 1);
    assert_eq!(data["version"], env!("CARGO_PKG_VERSION"));
}
