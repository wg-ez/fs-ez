use super::*;

#[tokio::test]
async fn views_crud_filter_json_query_and_paged_files() {
    let temp = TempDir::new().expect("tempdir");
    let state = state(&temp);
    let router = build_router(state.clone());
    let cookie = login(&router).await;
    create_tag(&router, &cookie, "facture").await;
    let file = seed_file(&state, &temp, "facture.txt");
    call(
        &router,
        Method::POST,
        &format!("/api/files/{}/tags", file.0),
        Some(&json!({ "tags": ["facture"], "action": "add" })),
        Some(&cookie),
    )
    .await;

    let reply = call(
        &router,
        Method::POST,
        "/api/views",
        Some(&json!({
            "name": "Factures",
            "query": "facture",
            "icon": "receipt",
            "color": "green",
            "sort": "name:desc"
        })),
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.text);
    assert_eq!(reply.json["query"], "facture");
    assert_eq!(reply.json["count"], 1);
    assert_eq!(reply.json["sort"]["field"], "name");
    assert_eq!(reply.json["sort"]["dir"], "desc");
    let id = reply.json["id"].as_i64().expect("id");

    let reply = call(
        &router,
        Method::POST,
        "/api/views",
        Some(&json!({
            "name": "Payées",
            "filter_json": { "op": "tag", "tag": "facture", "negated": true }
        })),
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.text);
    assert_eq!(reply.json["filter"]["op"], "tag");

    let reply = call(
        &router,
        Method::POST,
        "/api/views",
        Some(&json!({
            "name": "Divergente",
            "filter_json": { "op": "tag", "tag": "facture" },
            "query": "paye"
        })),
        Some(&cookie),
    )
    .await;
    assert_error(&reply, StatusCode::BAD_REQUEST, "bad_request");

    let reply = call(
        &router,
        Method::POST,
        "/api/views",
        Some(&json!({ "name": "Vide" })),
        Some(&cookie),
    )
    .await;
    assert_error(&reply, StatusCode::BAD_REQUEST, "bad_request");

    let reply = call(&router, Method::GET, "/api/views", None, Some(&cookie)).await;
    assert_eq!(reply.json["views"].as_array().map(Vec::len), Some(2));

    let reply = call(
        &router,
        Method::GET,
        &format!("/api/views/{id}/files?limit=1"),
        None,
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert_eq!(reply.json["items"].as_array().map(Vec::len), Some(1));

    let reply = call(
        &router,
        Method::PATCH,
        &format!("/api/views/{id}"),
        Some(&json!({ "name": "Renommée" })),
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert_eq!(reply.json["name"], "Renommée");
    assert_eq!(reply.json["query"], "facture");

    let reply = call(
        &router,
        Method::DELETE,
        &format!("/api/views/{id}"),
        None,
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.json["ok"], true);
    let reply = call(
        &router,
        Method::GET,
        &format!("/api/views/{id}"),
        None,
        Some(&cookie),
    )
    .await;
    assert_error(&reply, StatusCode::NOT_FOUND, "not_found");
}

#[tokio::test]
async fn patch_view_icon_and_color_null_clear_them() {
    let temp = TempDir::new().expect("tempdir");
    let router = build_router(state(&temp));
    let cookie = login(&router).await;

    let reply = call(
        &router,
        Method::POST,
        "/api/views",
        Some(&json!({
            "name": "Factures",
            "query": "facture",
            "icon": "receipt",
            "color": "green"
        })),
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.text);
    let id = reply.json["id"].as_i64().expect("id");
    assert_eq!(reply.json["icon"], "receipt");
    assert_eq!(reply.json["color"], "green");

    let reply = call(
        &router,
        Method::PATCH,
        &format!("/api/views/{id}"),
        Some(&json!({ "icon": null })),
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert!(reply.json["icon"].is_null(), "{}", reply.text);
    assert_eq!(reply.json["color"], "green");

    let reply = call(
        &router,
        Method::PATCH,
        &format!("/api/views/{id}"),
        Some(&json!({ "color": null })),
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert!(reply.json["color"].is_null(), "{}", reply.text);
}

#[tokio::test]
async fn patch_view_group_color_null_clears_it() {
    let temp = TempDir::new().expect("tempdir");
    let router = build_router(state(&temp));
    let cookie = login(&router).await;

    let reply = call(
        &router,
        Method::POST,
        "/api/view-groups",
        Some(&json!({ "name": "Docs", "color": "blue" })),
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.text);
    let id = reply.json["id"].as_i64().expect("id");

    let reply = call(
        &router,
        Method::PATCH,
        &format!("/api/view-groups/{id}"),
        Some(&json!({ "color": null })),
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert!(reply.json["color"].is_null(), "{}", reply.text);
}

#[tokio::test]
async fn negative_positions_and_view_ids_are_rejected() {
    let temp = TempDir::new().expect("tempdir");
    let router = build_router(state(&temp));
    let cookie = login(&router).await;

    let reply = call(
        &router,
        Method::POST,
        "/api/view-groups",
        Some(&json!({ "name": "Docs" })),
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.text);
    let group = reply.json["id"].as_i64().expect("id");

    let reply = call(
        &router,
        Method::PATCH,
        &format!("/api/view-groups/{group}"),
        Some(&json!({ "position": -1 })),
        Some(&cookie),
    )
    .await;
    assert_error(&reply, StatusCode::BAD_REQUEST, "bad_request");

    let reply = call(
        &router,
        Method::PATCH,
        &format!("/api/view-groups/{group}"),
        Some(&json!({ "position": 3 })),
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert_eq!(reply.json["position"], 3);

    let reply = call(
        &router,
        Method::POST,
        &format!("/api/view-groups/{group}/views"),
        Some(&json!({ "view": 0, "action": "assign" })),
        Some(&cookie),
    )
    .await;
    assert_error(&reply, StatusCode::BAD_REQUEST, "bad_request");

    let reply = call(
        &router,
        Method::POST,
        "/api/views",
        Some(&json!({ "name": "Vue", "query": "x", "position": -2 })),
        Some(&cookie),
    )
    .await;
    assert_error(&reply, StatusCode::BAD_REQUEST, "bad_request");
}

#[tokio::test]
async fn position_above_bound_is_rejected_without_blocking_creates() {
    let temp = TempDir::new().expect("tempdir");
    let router = build_router(state(&temp));
    let cookie = login(&router).await;

    for position in [1_000_000_001_i64, i64::MAX] {
        let reply = call(
            &router,
            Method::POST,
            "/api/views",
            Some(&json!({
                "name": format!("Trop {position}"),
                "query": "x",
                "position": position
            })),
            Some(&cookie),
        )
        .await;
        assert_error(&reply, StatusCode::BAD_REQUEST, "bad_request");
    }

    let reply = call(
        &router,
        Method::POST,
        "/api/views",
        Some(&json!({ "name": "Sans position", "query": "x" })),
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.text);

    let reply = call(
        &router,
        Method::POST,
        "/api/views",
        Some(&json!({ "name": "Position 3", "query": "x", "position": 3 })),
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.text);
    assert_eq!(reply.json["position"], 3);
}
