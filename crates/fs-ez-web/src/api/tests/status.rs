use super::*;

#[tokio::test]
async fn status_reports_counters_without_secrets() {
    let temp = TempDir::new().expect("tempdir");
    let state = state(&temp);
    let router = build_router(state.clone());
    let cookie = login(&router).await;
    create_tag(&router, &cookie, "facture").await;
    let file = seed_file(&state, &temp, "statut.txt");
    call(
        &router,
        Method::POST,
        &format!("/api/files/{}/tags", file.0),
        Some(&json!({ "tags": ["facture"], "action": "add" })),
        Some(&cookie),
    )
    .await;
    queue_file(&state, &temp, "en-attente.txt");
    let token = state
        .backend
        .app()
        .create_api_token("statut", None)
        .expect("jeton");

    let reply = send(
        &router,
        json_request(
            Method::GET,
            "/api/status",
            None,
            None,
            Some(&token.token),
            None,
        ),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert_eq!(reply.json["schema"], 1);
    assert_eq!(reply.json["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(reply.json["files"], 2);
    assert_eq!(reply.json["untagged"], 1);
    assert_eq!(reply.json["tags"], 1);
    assert_eq!(reply.json["views"], 0);
    assert!(reply.json["root"].is_string());
    assert!(reply.json["db_path"].is_string());
    for secret in ["token", "hash", "password", "secret", token.token.as_str()] {
        assert!(!reply.text.contains(secret), "{secret}");
    }
}
