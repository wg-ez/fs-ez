use super::*;

#[tokio::test]
async fn groups_crud_and_single_membership() {
    let temp = TempDir::new().expect("tempdir");
    let router = build_router(state(&temp));
    let cookie = login(&router).await;
    create_tag(&router, &cookie, "alpha").await;
    create_tag(&router, &cookie, "beta").await;

    let reply = call(
        &router,
        Method::POST,
        "/api/groups",
        Some(&json!({ "name": "Docs", "color": "teal" })),
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.text);
    assert_eq!(reply.json["name"], "Docs");
    assert_eq!(reply.json["color"], "teal");
    let first = reply.json["id"].as_i64().expect("id");

    let reply = call(
        &router,
        Method::POST,
        "/api/groups",
        Some(&json!({ "name": "Autre" })),
        Some(&cookie),
    )
    .await;
    let second = reply.json["id"].as_i64().expect("id");

    let reply = call(
        &router,
        Method::POST,
        &format!("/api/groups/{first}/tags"),
        Some(&json!({ "tag": "alpha", "action": "assign" })),
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert_eq!(reply.json["tags"].as_array().map(Vec::len), Some(1));

    let reply = call(
        &router,
        Method::POST,
        &format!("/api/groups/{second}/tags"),
        Some(&json!({ "tag": "alpha", "action": "assign" })),
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert_eq!(reply.json["tags"].as_array().map(Vec::len), Some(1));

    let reply = call(&router, Method::GET, "/api/groups", None, Some(&cookie)).await;
    assert_eq!(reply.status, StatusCode::OK);
    let groups = reply.json["groups"].as_array().expect("groupes");
    assert_eq!(groups.len(), 3);
    let docs = groups
        .iter()
        .find(|group| group["id"].as_i64() == Some(first))
        .expect("Docs");
    let autre = groups
        .iter()
        .find(|group| group["id"].as_i64() == Some(second))
        .expect("Autre");
    assert_eq!(docs["tags"].as_array().map(Vec::len), Some(0));
    assert_eq!(autre["tags"][0]["name"], "alpha");

    let reply = call(
        &router,
        Method::POST,
        &format!("/api/groups/{first}/tags"),
        Some(&json!({ "tag": "alpha", "action": "remove" })),
        Some(&cookie),
    )
    .await;
    assert_error(&reply, StatusCode::BAD_REQUEST, "bad_request");

    let reply = call(
        &router,
        Method::PATCH,
        &format!("/api/groups/{first}"),
        Some(&json!({ "name": "Documents", "collapsed": true })),
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert_eq!(reply.json["name"], "Documents");
    assert_eq!(reply.json["collapsed"], true);

    let reply = call(
        &router,
        Method::DELETE,
        &format!("/api/groups/{second}"),
        None,
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    let reply = call(&router, Method::GET, "/api/tags", None, Some(&cookie)).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert!(
        reply.json["tags"]
            .as_array()
            .expect("tags")
            .iter()
            .all(|tag| tag["name"] != "alpha")
    );

    let reply = call(
        &router,
        Method::POST,
        &format!("/api/groups/{first}/tags"),
        Some(&json!({ "tag": "absent", "action": "assign" })),
        Some(&cookie),
    )
    .await;
    assert_error(&reply, StatusCode::NOT_FOUND, "not_found");
}

#[tokio::test]
async fn patch_group_color_null_clears_it() {
    let temp = TempDir::new().expect("tempdir");
    let router = build_router(state(&temp));
    let cookie = login(&router).await;

    let reply = call(
        &router,
        Method::POST,
        "/api/groups",
        Some(&json!({ "name": "Docs", "color": "blue" })),
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.text);
    let id = reply.json["id"].as_i64().expect("id");

    let reply = call(
        &router,
        Method::PATCH,
        &format!("/api/groups/{id}"),
        Some(&json!({ "color": null })),
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert!(reply.json["color"].is_null(), "{}", reply.text);
}
