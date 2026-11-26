use super::*;

#[tokio::test]
async fn names_are_validated_for_length_presence_and_uniqueness() {
    let fixture = fixture().await;
    let response = create_view(&fixture, "", "", "facture").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(fixture.app.list_views().expect("vues").is_empty());

    let too_long = "a".repeat(101);
    let response = create_view(&fixture, &too_long, "", "facture").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(fixture.app.list_views().expect("vues").is_empty());

    let response = create_view(&fixture, "Unique", "", "facture").await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = create_view(&fixture, "Unique", "", "facture").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_text(response).await;
    assert!(body.contains("already used"), "{body}");
}

#[tokio::test]
async fn too_deep_and_too_wide_filters_are_rejected() {
    let fixture = fixture().await;
    let mut deep = FilterExpr::Tag {
        tag: "facture".to_string(),
        negated: false,
    };
    for _ in 0..33 {
        deep = FilterExpr::Not {
            item: Box::new(deep),
        };
    }
    let deep_json = serde_json::to_string(&deep).expect("json");
    let response = create_view(&fixture, "Profonde", &deep_json, "").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(fixture.app.list_views().expect("vues").is_empty());

    let wide = FilterExpr::And {
        items: (0..257)
            .map(|index| FilterExpr::Tag {
                tag: format!("t{index}"),
                negated: false,
            })
            .collect(),
    };
    let wide_json = serde_json::to_string(&wide).expect("json");
    let response = create_view(&fixture, "Large", &wide_json, "").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(fixture.app.list_views().expect("vues").is_empty());
}
