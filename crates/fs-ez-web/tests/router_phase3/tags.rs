use super::*;

#[tokio::test]
async fn tags_create_rename_keep_applications_change_color_and_delete() {
    let fixture = seeded().await;
    let file = fixture.file_id("scan-teal.txt");
    let group = fixture.group_id("Finance");
    let response = fixture
        .post_hx(
            "/partials/tags",
            &format!("name=ephemere&color=violet&group={group}"),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let tag = fixture
        .app
        .store
        .get_tag_by_name("ephemere")
        .expect("recherche")
        .expect("tag");
    assert_eq!(tag.color, TagColor::Violet);

    fixture
        .app
        .apply_tags(file, &["ephemere".to_string()], true)
        .expect("application");
    let id = fixture.tag_id("ephemere");
    let response = fixture
        .post_hx(&format!("/partials/tags/{id}/rename"), "name=durable")
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let applied: Vec<String> = fixture
        .app
        .file_with_tags(file)
        .expect("fichier")
        .tags
        .into_iter()
        .map(|tag| tag.tag.name)
        .collect();
    assert!(applied.contains(&"durable".to_string()), "{applied:?}");

    let response = fixture
        .post_hx(&format!("/partials/tags/{id}/color"), "color=pink")
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let tag = fixture
        .app
        .store
        .get_tag_by_name("durable")
        .expect("recherche")
        .expect("tag");
    assert_eq!(tag.color, TagColor::Pink);

    let response = fixture
        .post_hx(&format!("/partials/tags/{id}/delete"), "")
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        fixture
            .app
            .store
            .get_tag_by_name("durable")
            .expect("recherche")
            .is_none()
    );
    assert!(
        !fixture
            .app
            .file_with_tags(file)
            .expect("fichier")
            .tags
            .iter()
            .any(|tag| tag.tag.name == "durable")
    );
}
