use super::*;

#[tokio::test]
async fn classic_posts_without_javascript_redirect_with_303() {
    let fixture = seeded().await;

    let group = fixture.group_id("Finance");
    let response = fixture
        .post(
            "/partials/tags",
            &format!("name=sans-js&color=blue&group={group}"),
        )
        .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&response), "/");
    assert!(
        fixture
            .app
            .store
            .get_tag_by_name("sans-js")
            .expect("recherche")
            .is_some()
    );

    let body = form(&[("name", "Sans JS"), ("query", "facture")]);
    let response = fixture.post("/views", &body).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&response), "/");
    assert!(
        fixture
            .app
            .list_views()
            .expect("vues")
            .iter()
            .any(|view| view.name == "Sans JS")
    );

    let response = fixture
        .post(
            "/settings/preferences",
            "recent_days=14&page_size=20&sort=id:asc",
        )
        .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&response), "/");

    let (_, home) = fixture.page("/").await;
    assert!(home.contains("action=\"/views\""), "{home}");
    let (_, settings) = fixture.page("/settings").await;
    assert!(
        settings.contains("hx-post=\"/settings/preferences\""),
        "{settings}"
    );
}
