use super::*;

#[tokio::test]
async fn groups_assign_move_delete_and_union_counter() {
    let fixture = seeded().await;
    let response = fixture
        .post_hx("/partials/groups", "name=Union&color=blue")
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let union = fixture.group_id("Union");
    for tag in ["c-blue", "facture"] {
        let response = fixture
            .post_hx("/partials/tags/place", &format!("group={union}&tag={tag}"))
            .await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    let (_, sidebar) = fixture.page("/partials/sidebar").await;
    assert!(
        sidebar.contains(&format!("data-group=\"{union}\"")),
        "{sidebar}"
    );
    assert!(sidebar.contains("data-tag=\"facture\""), "{sidebar}");
    assert_eq!(sidebar.matches("data-tag=\"facture\"").count(), 1);

    let response = fixture.post_hx("/partials/groups", "name=Receveur").await;
    assert_eq!(response.status(), StatusCode::OK);
    let receiver = fixture.group_id("Receveur");
    let response = fixture
        .post_hx(
            "/partials/tags/place",
            &format!("group={receiver}&tag=facture"),
        )
        .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let source_tags: Vec<String> = fixture
        .app
        .tags_in_group(union)
        .expect("groupe")
        .into_iter()
        .map(|tag| tag.name)
        .collect();
    assert_eq!(source_tags, vec!["c-blue".to_string()]);
    assert_eq!(
        fixture.app.tags_in_group(receiver).expect("groupe")[0].name,
        "facture"
    );

    let response = fixture
        .post_hx(&format!("/partials/groups/{receiver}/delete"), "")
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        fixture
            .app
            .store
            .get_tag_by_name("facture")
            .expect("recherche")
            .is_none()
    );
}
