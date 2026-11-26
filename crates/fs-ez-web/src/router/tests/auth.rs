use super::*;

#[tokio::test]
async fn root_without_cookie_redirects_to_login() {
    let temp = TempDir::new().expect("tempdir");
    let router = build_router(state(&temp));
    let response = send(&router, get("/")).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&response), "/login?next=%2F");
}

#[tokio::test]
async fn api_without_cookie_returns_json_unauthorized() {
    let temp = TempDir::new().expect("tempdir");
    let router = build_router(state(&temp));
    let response = send(&router, get("/api/files")).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let json: Value = serde_json::from_str(&body_text(response).await).expect("json");
    assert_eq!(json["schema"], 1);
    assert_eq!(json["code"], "unauthorized");
    assert_eq!(json["error"], "authentication required");
}

#[tokio::test]
async fn successful_login_sets_cookie_and_grants_access() {
    let temp = TempDir::new().expect("tempdir");
    let router = build_router(state(&temp));
    let response = login(&router, None).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&response), "/");
    let cookie = set_cookie(&response);
    assert!(cookie.contains("HttpOnly"), "{cookie}");
    assert!(cookie.contains("SameSite=Lax"), "{cookie}");
    assert!(cookie.contains("Path=/"), "{cookie}");
    assert!(cookie.contains("Max-Age=2592000"), "{cookie}");
    assert!(cookie.contains("Secure"), "{cookie}");
    assert!(!cookie.contains("Domain"), "{cookie}");
    let token = session_token(&response);
    assert!(!token.is_empty());

    let response = send(&router, with_cookie(get("/"), &token)).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(body_text(response).await.contains("fs-ez"));

    let response = send(&router, with_cookie(get("/api/session"), &token)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let json: Value = serde_json::from_str(&body_text(response).await).expect("json");
    assert_eq!(json["schema"], 1);
    assert_eq!(json["username"], USERNAME);
}

#[tokio::test]
async fn secure_flag_follows_origin_scheme() {
    let temp = TempDir::new().expect("tempdir");
    let router = build_router(state(&temp));

    let response = send(
        &router,
        with_origin(
            post_form("/login", &login_body(PASSWORD)),
            "http://127.0.0.1:4242",
        ),
    )
    .await;
    assert!(!set_cookie(&response).contains("Secure"));

    let response = send(
        &router,
        with_origin(
            post_form("/login", &login_body(PASSWORD)),
            "https://fs.local",
        ),
    )
    .await;
    assert!(set_cookie(&response).contains("Secure"));

    let response = login(&router, None).await;
    assert!(set_cookie(&response).contains("Secure"));
}

#[tokio::test]
async fn forged_cookie_is_rejected() {
    let temp = TempDir::new().expect("tempdir");
    let router = build_router(state(&temp));
    let response = send(&router, with_cookie(get("/"), "falsifie")).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let response = send(&router, with_cookie(get("/api/session"), "falsifie")).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn logout_clears_cookie_and_deletes_session() {
    let temp = TempDir::new().expect("tempdir");
    let shared = state(&temp);
    let app = shared.backend.app();
    let router = build_router(shared);
    let response = login(&router, None).await;
    let token = session_token(&response);
    assert!(app.validate_session(&token).expect("session").is_some());

    let response = send(&router, with_cookie(post_form("/logout", ""), &token)).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&response), "/login");
    assert!(set_cookie(&response).contains("Max-Age=0"));
    assert!(app.validate_session(&token).expect("session").is_none());
}

#[tokio::test]
async fn five_failures_then_too_many_requests() {
    let temp = TempDir::new().expect("tempdir");
    let router = build_router(state(&temp));
    for attempt in 0..5 {
        let response = send(
            &router,
            post_form("/login", &login_body("mauvais-mot-de-passe")),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK, "tentative {attempt}");
    }
    let response = send(
        &router,
        post_form("/login", &login_body("mauvais-mot-de-passe")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        response
            .headers()
            .get(header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok()),
        Some("900")
    );
}

#[tokio::test]
async fn unsafe_next_targets_are_ignored() {
    let temp = TempDir::new().expect("tempdir");
    let router = build_router(state(&temp));
    for target in ["//evil.example", "https://evil.example"] {
        let response = login(&router, Some(target)).await;
        assert_eq!(response.status(), StatusCode::SEE_OTHER, "{target}");
        assert_eq!(location(&response), "/", "{target}");
    }
    let response = login(&router, Some("/inbox")).await;
    assert_eq!(location(&response), "/inbox");
}

#[tokio::test]
async fn unknown_route_requires_authentication() {
    let temp = TempDir::new().expect("tempdir");
    let router = build_router(state(&temp));
    let response = send(&router, get("/inconnu")).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);

    let response = login(&router, None).await;
    let token = session_token(&response);
    let response = send(&router, with_cookie(get("/inconnu"), &token)).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let json: Value = serde_json::from_str(&body_text(response).await).expect("json");
    assert_eq!(json["schema"], 1);
    assert_eq!(json["code"], "not_found");
}
