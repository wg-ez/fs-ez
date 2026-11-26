use super::*;

#[tokio::test]
async fn invalid_bodies_ids_and_limits_are_bad_requests() {
    let temp = TempDir::new().expect("tempdir");
    let router = build_router(state(&temp));
    let cookie = login(&router).await;

    let reply = send(
        &router,
        Request::builder()
            .method(Method::POST)
            .uri("/api/tags")
            .header(header::CONTENT_TYPE, "application/json")
            .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 4242))))
            .header(header::COOKIE, format!("fs-ez_session={cookie}"))
            .body(Body::from("{pas du json"))
            .expect("requête"),
    )
    .await;
    assert_error(&reply, StatusCode::BAD_REQUEST, "bad_request");

    let cases = [
        ("/api/files/abc", Method::GET),
        ("/api/files?limit=0", Method::GET),
        ("/api/files?limit=501", Method::GET),
        ("/api/files?limit=abc", Method::GET),
        ("/api/files?sort=date", Method::GET),
        ("/api/files?filter=bogus", Method::GET),
        ("/api/files?query=facture%20and", Method::GET),
        ("/api/views/abc", Method::GET),
        ("/api/groups/abc", Method::DELETE),
    ];
    for (uri, method) in cases {
        let reply = call(&router, method, uri, None, Some(&cookie)).await;
        assert_error(&reply, StatusCode::BAD_REQUEST, "bad_request");
    }

    let reply = call(
        &router,
        Method::POST,
        "/api/files/1/tags",
        Some(&json!({ "tags": ["x"], "action": "bogus" })),
        Some(&cookie),
    )
    .await;
    assert_error(&reply, StatusCode::BAD_REQUEST, "bad_request");

    let reply = call(
        &router,
        Method::POST,
        "/api/files/1/tags",
        Some(&json!({ "tags": [], "action": "add" })),
        Some(&cookie),
    )
    .await;
    assert_error(&reply, StatusCode::BAD_REQUEST, "bad_request");
}

#[tokio::test]
async fn oversized_json_body_is_payload_too_large() {
    let temp = TempDir::new().expect("tempdir");
    let router = build_router(state(&temp));
    let cookie = login(&router).await;
    let payload = json!({ "name": "gros", "padding": "a".repeat(3 * 1024 * 1024) });
    let reply = call(
        &router,
        Method::POST,
        "/api/tags",
        Some(&payload),
        Some(&cookie),
    )
    .await;
    assert_error(&reply, StatusCode::PAYLOAD_TOO_LARGE, "payload_too_large");
}

#[tokio::test]
async fn success_responses_are_json_and_never_serve_file_content() {
    let temp = TempDir::new().expect("tempdir");
    let state = state(&temp);
    let router = build_router(state.clone());
    let cookie = login(&router).await;
    let file = seed_file(&state, &temp, "secret.txt");

    for uri in ["/api/files", "/api/tags", "/api/views", "/api/groups"] {
        let reply = call(&router, Method::GET, uri, None, Some(&cookie)).await;
        assert_eq!(reply.status, StatusCode::OK, "{uri}");
        let content_type = reply
            .headers
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        assert!(content_type.starts_with("application/json"), "{uri}");
        assert_eq!(reply.json["schema"], 1, "{uri}");
        assert!(reply.headers.get(header::CONTENT_DISPOSITION).is_none());
        assert!(reply.headers.get(header::ACCEPT_RANGES).is_none());
        assert!(!reply.text.contains("contenu secret"), "{uri}");
    }

    let reply = call(
        &router,
        Method::GET,
        &format!("/api/files/{}", file.0),
        None,
        Some(&cookie),
    )
    .await;
    assert!(!reply.text.contains("contenu secret"));
}
