use super::*;

#[tokio::test]
async fn tags_round_trip() {
    let temp = TempDir::new().expect("tempdir");
    let router = build_router(state(&temp));
    let cookie = login(&router).await;

    let reply = call(
        &router,
        Method::POST,
        "/api/tags",
        Some(&json!({ "name": "facture", "color": "blue" })),
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.text);
    assert_eq!(reply.json["name"], "facture");
    assert_eq!(reply.json["color"], "blue");
    assert_eq!(reply.json["schema"], 1);

    let reply = call(
        &router,
        Method::PATCH,
        "/api/tags/facture",
        Some(&json!({ "name": "invoice", "color": "amber", "color_custom": "#AABBCC" })),
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert_eq!(reply.json["name"], "invoice");
    assert_eq!(reply.json["color"], "amber");
    assert_eq!(reply.json["color_custom"], "#aabbcc");

    let reply = call(&router, Method::GET, "/api/tags", None, Some(&cookie)).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.json["tags"].as_array().map(Vec::len), Some(1));
    assert_eq!(reply.json["tags"][0]["count"], 0);

    create_tag(&router, &cookie, "alpha").await;
    create_tag(&router, &cookie, "beta").await;
    let reply = call(
        &router,
        Method::PATCH,
        "/api/tags/beta",
        Some(&json!({ "name": "alpha" })),
        Some(&cookie),
    )
    .await;
    assert_error(&reply, StatusCode::CONFLICT, "conflict");

    let reply = call(
        &router,
        Method::PATCH,
        "/api/tags/alpha",
        Some(&json!({ "color": "magenta" })),
        Some(&cookie),
    )
    .await;
    assert_error(&reply, StatusCode::BAD_REQUEST, "bad_request");

    let reply = call(
        &router,
        Method::DELETE,
        "/api/tags/invoice",
        None,
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert_eq!(reply.json["ok"], true);
    let reply = call(
        &router,
        Method::DELETE,
        "/api/tags/invoice",
        None,
        Some(&cookie),
    )
    .await;
    assert_error(&reply, StatusCode::NOT_FOUND, "not_found");
}

#[tokio::test]
async fn patch_tag_color_custom_is_tristate() {
    let temp = TempDir::new().expect("tempdir");
    let router = build_router(state(&temp));
    let cookie = login(&router).await;
    create_tag(&router, &cookie, "facture").await;

    let reply = call(
        &router,
        Method::PATCH,
        "/api/tags/facture",
        Some(&json!({ "color": "blue", "color_custom": "#aabbcc" })),
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert_eq!(reply.json["color"], "blue");
    assert_eq!(reply.json["color_custom"], "#aabbcc");

    let reply = call(
        &router,
        Method::PATCH,
        "/api/tags/facture",
        Some(&json!({ "color": "red" })),
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert_eq!(reply.json["color"], "red");
    assert_eq!(reply.json["color_custom"], "#aabbcc", "{}", reply.text);

    let reply = call(
        &router,
        Method::PATCH,
        "/api/tags/facture",
        Some(&json!({ "color": "blue" })),
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert_eq!(reply.json["color"], "blue");
    assert_eq!(reply.json["color_custom"], "#aabbcc", "{}", reply.text);

    let reply = call(
        &router,
        Method::PATCH,
        "/api/tags/facture",
        Some(&json!({ "color_custom": null })),
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert_eq!(reply.json["color"], "blue");
    assert!(reply.json["color_custom"].is_null(), "{}", reply.text);

    let reply = call(
        &router,
        Method::PATCH,
        "/api/tags/facture",
        Some(&json!({ "color_custom": "#zzzzzz" })),
        Some(&cookie),
    )
    .await;
    assert_error(&reply, StatusCode::BAD_REQUEST, "bad_request");
}

#[tokio::test]
async fn create_tag_rejects_non_positive_group() {
    let temp = TempDir::new().expect("tempdir");
    let router = build_router(state(&temp));
    let cookie = login(&router).await;

    for group in [0, -1] {
        let reply = call(
            &router,
            Method::POST,
            "/api/tags",
            Some(&json!({ "name": "g", "group": group })),
            Some(&cookie),
        )
        .await;
        assert_error(&reply, StatusCode::BAD_REQUEST, "bad_request");
    }
}
