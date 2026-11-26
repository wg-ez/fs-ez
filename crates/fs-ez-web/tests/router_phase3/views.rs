use super::*;

#[tokio::test]
async fn four_views_run_with_their_filter_and_sort() {
    let fixture = seeded().await;
    let (status, sidebar) = fixture.page("/partials/sidebar").await;
    assert_eq!(status, StatusCode::OK);
    for name in ["Factures", "Projets", "Archives", "Tout"] {
        assert!(sidebar.contains(&format!("title=\"{name}\"")), "{sidebar}");
    }

    let (status, html) = fixture
        .page(&format!("/views/{}", fixture.view_id("Factures")))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("facture-due.txt"), "{html}");
    assert!(!html.contains("facture-payee.txt"), "{html}");

    let (status, html) = fixture
        .page(&format!("/views/{}", fixture.view_id("Projets")))
        .await;
    assert_eq!(status, StatusCode::OK);
    let first = html.find("projet-alpha.txt").expect("projet");
    let second = html.find("facture-due.txt").expect("facture");
    assert!(first < second, "tri id descendant attendu : {html}");

    let (status, html) = fixture
        .page(&format!("/views/{}", fixture.view_id("Archives")))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("archive-2025.txt"), "{html}");
    assert!(!html.contains("facture-due.txt"), "{html}");

    let (status, html) = fixture
        .page(&format!("/views/{}", fixture.view_id("Tout")))
        .await;
    assert_eq!(status, StatusCode::OK);
    for name in ["facture-due.txt", "facture-payee.txt", "archive-2025.txt"] {
        assert!(html.contains(name), "{name}");
    }
}

#[tokio::test]
async fn builder_writes_canonical_query_and_incoherent_input_is_rejected() {
    let fixture = seeded().await;
    let filter = serde_json::to_string(&FilterExpr::And {
        items: vec![tag_filter("facture", false), tag_filter("paye", true)],
    })
    .expect("json");
    let body = form(&[
        ("name", "Builder"),
        ("filter_json", &filter),
        ("query", ""),
        ("sort", "name:asc"),
    ]);
    let response = fixture.post_hx("/views", &body).await;
    assert_eq!(response.status(), StatusCode::OK);
    let builder = fixture
        .app
        .list_views()
        .expect("vues")
        .into_iter()
        .find(|view| view.name == "Builder")
        .expect("vue");
    assert_eq!(
        fixture
            .app
            .canonical_filter(&builder.filter)
            .expect("requête"),
        "facture and not paye"
    );

    let body = form(&[
        ("name", "Incoherente"),
        ("filter_json", &filter),
        ("query", "paye"),
        ("sort", "name:asc"),
    ]);
    let response = fixture.post_hx("/views", &body).await;
    assert_eq!(response.status(), StatusCode::OK);
    let text = body_text(response).await;
    assert!(text.contains("diverge"), "{text}");
    assert!(
        fixture
            .app
            .list_views()
            .expect("vues")
            .iter()
            .all(|view| view.name != "Incoherente")
    );
}
