use super::*;

#[tokio::test]
async fn every_route_requires_authentication() {
    let temp = TempDir::new().expect("tempdir");
    let router = build_router(state(&temp));
    let routes = [
        (Method::GET, "/api/files"),
        (Method::GET, "/api/files/1"),
        (Method::POST, "/api/files/1/tags"),
        (Method::DELETE, "/api/files/1/tags/facture"),
        (Method::GET, "/api/tags"),
        (Method::POST, "/api/tags"),
        (Method::PATCH, "/api/tags/facture"),
        (Method::DELETE, "/api/tags/facture"),
        (Method::GET, "/api/views"),
        (Method::POST, "/api/views"),
        (Method::GET, "/api/views/1"),
        (Method::PATCH, "/api/views/1"),
        (Method::DELETE, "/api/views/1"),
        (Method::GET, "/api/views/1/files"),
        (Method::GET, "/api/groups"),
        (Method::POST, "/api/groups"),
        (Method::PATCH, "/api/groups/1"),
        (Method::DELETE, "/api/groups/1"),
        (Method::POST, "/api/groups/1/tags"),
        (Method::POST, "/api/inbox/scan"),
        (Method::GET, "/api/tokens"),
        (Method::POST, "/api/tokens"),
        (Method::DELETE, "/api/tokens/1"),
        (Method::GET, "/api/status"),
        (Method::GET, "/api/events"),
    ];
    for (method, uri) in routes {
        let reply = call(&router, method.clone(), uri, None, None).await;
        assert_error(&reply, StatusCode::UNAUTHORIZED, "unauthorized");
        assert_eq!(reply.json["error"], "authentication required");
    }
}

#[tokio::test]
async fn cross_site_mutations_are_forbidden_with_a_session() {
    let temp = TempDir::new().expect("tempdir");
    let router = build_router(state(&temp));
    let cookie = login(&router).await;
    let routes = [
        (Method::POST, "/api/files/1/tags"),
        (Method::DELETE, "/api/files/1/tags/facture"),
        (Method::POST, "/api/tags"),
        (Method::PATCH, "/api/tags/facture"),
        (Method::DELETE, "/api/tags/facture"),
        (Method::POST, "/api/views"),
        (Method::PATCH, "/api/views/1"),
        (Method::DELETE, "/api/views/1"),
        (Method::POST, "/api/groups"),
        (Method::PATCH, "/api/groups/1"),
        (Method::DELETE, "/api/groups/1"),
        (Method::POST, "/api/groups/1/tags"),
    ];
    for (method, uri) in routes {
        let reply = send(
            &router,
            json_request(
                method,
                uri,
                Some(&json!({})),
                Some(&cookie),
                None,
                Some("cross-site"),
            ),
        )
        .await;
        assert_error(&reply, StatusCode::FORBIDDEN, "csrf");
    }
}

#[tokio::test]
async fn bearer_token_grants_access_and_rejections_are_unauthorized() {
    let temp = TempDir::new().expect("tempdir");
    let state = state(&temp);
    let app = state.backend.app();
    let created = app.create_api_token("ci", None).expect("jeton");
    let router = build_router(state.clone());

    let reply = send(
        &router,
        json_request(
            Method::GET,
            "/api/files",
            None,
            None,
            Some(&created.token),
            None,
        ),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert_eq!(reply.json["schema"], 1);

    let reply = send(
        &router,
        json_request(
            Method::POST,
            "/api/tags",
            Some(&json!({ "name": "viajeton" })),
            None,
            Some(&created.token),
            None,
        ),
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.text);
    assert_eq!(reply.json["name"], "viajeton");

    // CSRF ne concerne pas un jeton Bearer : l'en-tête Sec-Fetch-Site est ignoré.
    let reply = send(
        &router,
        json_request(
            Method::POST,
            "/api/tags",
            Some(&json!({ "name": "croise" })),
            None,
            Some(&created.token),
            Some("cross-site"),
        ),
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.text);

    for raw in ["Basic abc", "Bearer", "Bearer ", "pas-un-jeton"] {
        let reply = send(
            &router,
            Request::builder()
                .method(Method::GET)
                .uri("/api/files")
                .header(header::AUTHORIZATION, raw)
                .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 4242))))
                .body(Body::empty())
                .expect("requête"),
        )
        .await;
        assert_error(&reply, StatusCode::UNAUTHORIZED, "unauthorized");
    }

    let expired = "A".repeat(43);
    let now = time::OffsetDateTime::now_utc();
    let generation = app
        .store
        .get_user()
        .expect("user")
        .expect("ligne")
        .credential_generation;
    app.store
        .create_api_token(
            "expired",
            &fs_ez_app::token::hash_token(&expired),
            generation,
            now - time::Duration::hours(2),
            Some(now - time::Duration::hours(1)),
        )
        .expect("jeton expiré");
    let reply = send(
        &router,
        json_request(Method::GET, "/api/files", None, None, Some(&expired), None),
    )
    .await;
    assert_error(&reply, StatusCode::UNAUTHORIZED, "unauthorized");

    let revoked = app.create_api_token("revoked", None).expect("jeton");
    app.revoke_api_token(revoked.record.id).expect("révocation");
    let reply = send(
        &router,
        json_request(
            Method::GET,
            "/api/files",
            None,
            None,
            Some(&revoked.token),
            None,
        ),
    )
    .await;
    assert_error(&reply, StatusCode::UNAUTHORIZED, "unauthorized");
}
