use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};

use super::*;

#[tokio::test]
async fn files_listing_tagging_and_pagination() {
    let temp = TempDir::new().expect("tempdir");
    let state = state(&temp);
    let router = build_router(state.clone());
    let cookie = login(&router).await;
    create_tag(&router, &cookie, "facture").await;
    create_tag(&router, &cookie, "paye").await;

    let first = seed_file(&state, &temp, "a.txt");
    let second = seed_file(&state, &temp, "b.txt");
    let _third = seed_file(&state, &temp, "c.txt");

    let reply = call(
        &router,
        Method::POST,
        &format!("/api/files/{}/tags", first.0),
        Some(&json!({ "tags": ["facture", "year"], "action": "add" })),
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    let tags = reply.json["tags"].as_array().expect("tags");
    assert!(
        tags.iter()
            .any(|tag| tag["name"] == "facture" && tag["explicit"] == true)
    );
    assert!(tags.iter().any(|tag| tag["name"] == "year"));
    assert!(reply.json.get("content").is_none());

    let reply = call(
        &router,
        Method::GET,
        "/api/files?tag=facture",
        None,
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.json["items"].as_array().map(Vec::len), Some(1));
    assert_eq!(reply.json["items"][0]["id"], first.0);
    assert_eq!(
        reply.json["items"][0]["path"].as_str(),
        Some(temp.path().join("a.txt").to_string_lossy().as_ref())
    );

    let reply = call(
        &router,
        Method::GET,
        "/api/files?query=facture%20and%20not%20paye",
        None,
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert_eq!(reply.json["items"].as_array().map(Vec::len), Some(1));

    let reply = call(
        &router,
        Method::GET,
        "/api/files?q=a&query=facture",
        None,
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);

    let reply = call(
        &router,
        Method::GET,
        "/api/files?query=facture&tag=paye",
        None,
        Some(&cookie),
    )
    .await;
    assert_error(&reply, StatusCode::BAD_REQUEST, "bad_request");

    let filter = utf8_percent_encode(r#"{"op":"tag","tag":"facture"}"#, NON_ALPHANUMERIC);
    let reply = call(
        &router,
        Method::GET,
        &format!("/api/files?filter_json={filter}&tag=year"),
        None,
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert_eq!(reply.json["items"].as_array().map(Vec::len), Some(1));

    let reply = call(
        &router,
        Method::DELETE,
        &format!("/api/files/{}/tags/year", first.0),
        None,
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert!(
        !reply.json["tags"]
            .as_array()
            .expect("tags")
            .iter()
            .any(|tag| tag["name"] == "year")
    );

    let reply = call(
        &router,
        Method::POST,
        &format!("/api/files/{}/tags", first.0),
        Some(&json!({ "tags": ["year"], "action": "remove" })),
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);

    let reply = call(
        &router,
        Method::POST,
        &format!("/api/files/{}/tags", first.0),
        Some(&json!({ "tags": ["paye"], "action": "set" })),
        Some(&cookie),
    )
    .await;
    let names: Vec<String> = reply.json["tags"]
        .as_array()
        .expect("tags")
        .iter()
        .map(|tag| tag["name"].as_str().unwrap_or_default().to_string())
        .collect();
    assert_eq!(names, vec!["paye"]);

    let reply = call(
        &router,
        Method::GET,
        "/api/files?limit=2",
        None,
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.json["items"].as_array().map(Vec::len), Some(2));
    let offset = reply.json["next_offset"]
        .as_u64()
        .expect("offset")
        .to_string();
    let reply = call(
        &router,
        Method::GET,
        &format!("/api/files?limit=2&offset={offset}"),
        None,
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.json["items"].as_array().map(Vec::len), Some(1));

    let reply = call(
        &router,
        Method::GET,
        &format!("/api/files/{}", second.0),
        None,
        Some(&cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.json["name"], "b.txt");
    assert!(reply.headers.get(header::CONTENT_DISPOSITION).is_none());
    assert!(reply.headers.get(header::ACCEPT_RANGES).is_none());
    assert!(!reply.text.contains("contenu secret"));

    let reply = call(&router, Method::GET, "/api/files/9999", None, Some(&cookie)).await;
    assert_error(&reply, StatusCode::NOT_FOUND, "not_found");

    let reply = call(
        &router,
        Method::POST,
        "/api/files/9999/tags",
        Some(&json!({ "tags": ["paye"], "action": "add" })),
        Some(&cookie),
    )
    .await;
    assert_error(&reply, StatusCode::NOT_FOUND, "not_found");
}
