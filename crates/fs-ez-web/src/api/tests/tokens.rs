use super::*;

#[tokio::test]
async fn api_tokens_are_session_only_and_plaintext_is_returned_once() {
    let temp = TempDir::new().expect("tempdir");
    let state = state(&temp);
    let router = build_router(state.clone());
    let cookie = login(&router).await;

    let reply = call(
        &router,
        Method::POST,
        "/api/tokens",
        Some(&json!({ "name": " ci ", "expires_at": null })),
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.text);
    assert_eq!(reply.json["name"], "ci");
    assert!(
        reply.json["token"]
            .as_str()
            .is_some_and(|value| value.len() == 43)
    );
    assert!(reply.json.get("token_hash").is_none());
    let token_id = reply.json["id"].as_i64().expect("id");
    let plaintext = reply.json["token"].as_str().expect("token").to_string();

    let reply = call(
        &router,
        Method::POST,
        "/api/tokens",
        Some(&json!({ "name": "ci" })),
        Some(&cookie),
    )
    .await;
    assert_error(&reply, StatusCode::CONFLICT, "conflict");

    let reply = call(
        &router,
        Method::POST,
        "/api/tokens",
        Some(&json!({ "name": "passe", "expires_at": "2020-01-01T00:00:00Z" })),
        Some(&cookie),
    )
    .await;
    assert_error(&reply, StatusCode::BAD_REQUEST, "bad_request");

    let reply = call(
        &router,
        Method::POST,
        "/api/tokens",
        Some(&json!({ "name": "invalide", "expires_at": "demain" })),
        Some(&cookie),
    )
    .await;
    assert_error(&reply, StatusCode::BAD_REQUEST, "bad_request");

    let reply = call(&router, Method::GET, "/api/tokens", None, Some(&cookie)).await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert_eq!(reply.json["tokens"].as_array().map(Vec::len), Some(1));
    assert!(reply.json["tokens"][0].get("token").is_none());
    assert!(reply.json["tokens"][0].get("token_hash").is_none());
    assert!(reply.json["tokens"][0]["created_at"].is_string());

    for (method, uri) in [
        (Method::GET, "/api/tokens".to_string()),
        (Method::POST, "/api/tokens".to_string()),
        (Method::DELETE, format!("/api/tokens/{token_id}")),
    ] {
        let body = (method == Method::POST).then(|| json!({ "name": "autre" }));
        let reply = send(
            &router,
            json_request(method, &uri, body.as_ref(), None, Some(&plaintext), None),
        )
        .await;
        assert_error(&reply, StatusCode::FORBIDDEN, "forbidden");
    }

    let reply = call(
        &router,
        Method::DELETE,
        &format!("/api/tokens/{token_id}"),
        None,
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert_eq!(reply.json["ok"], true);

    let reply = call(
        &router,
        Method::DELETE,
        &format!("/api/tokens/{token_id}"),
        None,
        Some(&cookie),
    )
    .await;
    assert_error(&reply, StatusCode::NOT_FOUND, "not_found");

    let reply = call(
        &router,
        Method::DELETE,
        "/api/tokens/abc",
        None,
        Some(&cookie),
    )
    .await;
    assert_error(&reply, StatusCode::BAD_REQUEST, "bad_request");
}
