use super::*;

#[tokio::test]
async fn create_from_filter_json_lists_matching_files() {
    let fixture = fixture().await;
    fixture.add_file("facture-due.txt", &["facture"]);
    fixture.add_file("facture-payee.txt", &["facture", "paye"]);

    let response = create_view(&fixture, "Factures à payer", &facture_not_paye_json(), "").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        header_string(&response, header::HeaderName::from_static("hx-trigger")),
        "views-changed"
    );
    let html = body_text(response).await;
    assert!(html.contains("Factures à payer"), "{html}");
    assert!(html.contains("hx-swap-oob=\"outerHTML\""), "{html}");
    assert!(html.contains("id=\"sidebar\""), "{html}");

    let views = fixture.app.list_views().expect("vues");
    assert_eq!(views.len(), 1);
    assert_eq!(
        fixture
            .app
            .canonical_filter(&views[0].filter)
            .expect("requête"),
        "facture and not paye"
    );
    assert_eq!(fixture.app.count_view(views[0].id).expect("compte"), 1);

    let (status, sidebar) = fixture.page("/partials/sidebar").await;
    assert_eq!(status, StatusCode::OK);
    assert!(sidebar.contains("Factures à payer"), "{sidebar}");
    assert!(
        sidebar.contains(&format!("href=\"/views/{}\"", views[0].id)),
        "{sidebar}"
    );

    let (status, results) = fixture.page(&format!("/views/{}", views[0].id)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(results.contains("facture-due.txt"), "{results}");
    assert!(!results.contains("facture-payee.txt"), "{results}");
}

#[tokio::test]
async fn only_tag_view_excludes_files_with_other_tags() {
    let fixture = fixture().await;
    fixture.add_file("only.html", &["musique"]);
    fixture.add_file("mixed.html", &["musique", "network"]);
    let filter = serde_json::to_string(&FilterExpr::OnlyTag {
        tag: "musique".to_string(),
        negated: false,
    })
    .expect("json");

    let response = create_view(&fixture, "Musique seule", &filter, "").await;
    assert_eq!(response.status(), StatusCode::OK);
    let view = fixture.app.list_views().expect("vues").remove(0);
    assert_eq!(fixture.app.count_view(view.id).expect("compte"), 1);

    let (status, results) = fixture.page(&format!("/views/{}", view.id)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(results.contains("only.html"), "{results}");
    assert!(!results.contains("mixed.html"), "{results}");
}

#[tokio::test]
async fn exact_tag_set_view_excludes_extra_tags() {
    let fixture = fixture().await;
    fixture.add_file("network-software.txt", &["network", "software"]);
    fixture.add_file("mixed.txt", &["network", "software", "musique"]);
    fixture.add_file("network-only.txt", &["network"]);
    fixture.add_file("musique-only.txt", &["musique"]);
    let filter = serde_json::to_string(&FilterExpr::Or {
        items: vec![
            FilterExpr::TagSet {
                tags: vec!["network".to_string(), "software".to_string()],
                negated: false,
            },
            FilterExpr::TagSet {
                tags: vec!["musique".to_string()],
                negated: false,
            },
        ],
    })
    .expect("json");

    let response = create_view(&fixture, "Exact", &filter, "").await;
    assert_eq!(response.status(), StatusCode::OK);
    let view = fixture.app.list_views().expect("vues").remove(0);
    assert_eq!(fixture.app.count_view(view.id).expect("compte"), 2);

    let (status, results) = fixture.page(&format!("/views/{}", view.id)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(results.contains("network-software.txt"), "{results}");
    assert!(results.contains("musique-only.txt"), "{results}");
    assert!(!results.contains("mixed.txt"), "{results}");
    assert!(!results.contains("network-only.txt"), "{results}");
}

#[tokio::test]
async fn create_from_query_only_builds_the_filter_tree() {
    let fixture = fixture().await;
    let response = create_view(&fixture, "Payées", "", "facture and not paye").await;
    assert_eq!(response.status(), StatusCode::OK);
    let view = fixture.app.list_views().expect("vues").remove(0);
    assert_eq!(
        view.filter,
        FilterExpr::And {
            items: vec![
                FilterExpr::Tag {
                    tag: "facture".to_string(),
                    negated: false,
                },
                FilterExpr::Tag {
                    tag: "paye".to_string(),
                    negated: true,
                },
            ],
        }
    );
}

#[tokio::test]
async fn coherent_filter_and_query_are_accepted_but_different_ones_are_rejected() {
    let fixture = fixture().await;
    let response = create_view(
        &fixture,
        "Cohérente",
        &facture_not_paye_json(),
        "facture and not paye",
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let response = create_view(&fixture, "Incohérente", &facture_not_paye_json(), "paye").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_text(response).await;
    assert!(body.contains("diverge"), "{body}");
    assert_eq!(fixture.app.list_views().expect("vues").len(), 1);
}
