use super::*;

#[tokio::test]
async fn groups_and_tags_can_be_reordered() {
    let fixture = fixture().await;
    let first = fixture.app.create_group("Premier", None).expect("groupe");
    let second = fixture.app.create_group("Second", None).expect("groupe");
    let third = fixture.app.create_group("Troisieme", None).expect("groupe");

    let response = fixture
        .post_hx(
            "/partials/groups/reorder",
            &format!("ids={},{},{}", third.id, first.id, second.id),
        )
        .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let names: Vec<String> = fixture
        .app
        .list_groups()
        .expect("groupes")
        .into_iter()
        .map(|group| group.name)
        .collect();
    assert_eq!(names, vec!["Troisieme", "Premier", "Second"]);

    for name in ["un", "deux", "trois"] {
        fixture.create_tag(name, TagColor::Neutral);
    }
    for name in ["un", "deux", "trois"] {
        fixture.app.assign_tag(first.id, name).expect("assignation");
    }
    let tags = fixture.app.tags_in_group(first.id).expect("tags");
    let un = tags.iter().find(|tag| tag.name == "un").expect("un");
    let response = fixture
        .post_hx(
            "/partials/tags/place",
            &format!("group={}&tag=trois&before={}", first.id, un.id.0),
        )
        .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let names: Vec<String> = fixture
        .app
        .tags_in_group(first.id)
        .expect("tags")
        .into_iter()
        .map(|tag| tag.name)
        .collect();
    assert_eq!(names, vec!["trois", "un", "deux"]);
}

#[tokio::test]
async fn group_reorder_rejects_the_old_ungrouped_token() {
    let fixture = fixture().await;
    let first = fixture.app.create_group("Premier", None).expect("groupe");
    let second = fixture.app.create_group("Second", None).expect("groupe");

    let response = fixture
        .post_hx(
            "/partials/groups/reorder",
            &format!("ids={},ungrouped,{}", first.id, second.id),
        )
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn tags_can_move_between_groups() {
    let fixture = fixture().await;
    let first = fixture.app.create_group("Premier", None).expect("groupe");
    let second = fixture.app.create_group("Second", None).expect("groupe");
    for name in ["alpha", "beta", "gamma"] {
        fixture.create_tag(name, TagColor::Neutral);
        fixture.app.assign_tag(first.id, name).expect("assignation");
    }

    let (status, sidebar) = fixture.page("/partials/sidebar").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(sidebar.matches("data-tag=\"beta\"").count(), 1, "{sidebar}");

    let response = fixture
        .post_hx(
            "/partials/tags/place",
            &format!("group={}&tag=beta", second.id),
        )
        .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(
        fixture
            .app
            .tags_in_group(first.id)
            .expect("groupe")
            .iter()
            .all(|tag| tag.name != "beta")
    );
    assert_eq!(
        fixture.app.tags_in_group(second.id).expect("groupe").len(),
        1
    );

    let (_, sidebar) = fixture.page("/partials/sidebar").await;
    assert_eq!(sidebar.matches("data-tag=\"beta\"").count(), 1, "{sidebar}");
}

#[tokio::test]
async fn deleting_a_group_deletes_its_tags() {
    let fixture = fixture().await;
    let group = fixture
        .app
        .create_group("Temporaire", None)
        .expect("groupe");
    fixture.create_tag("libre", TagColor::Teal);
    fixture
        .app
        .assign_tag(group.id, "libre")
        .expect("assignation");

    let response = fixture
        .post_hx(&format!("/partials/groups/{}/delete", group.id), "")
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_fragment_response(&response);
    assert!(fixture.app.list_groups().expect("groupes").is_empty());
    assert!(fixture.app.list_tags().expect("tags").is_empty());

    let (status, html) = fixture.page("/partials/sidebar").await;
    assert_eq!(status, StatusCode::OK);
    assert!(!html.contains("libre"), "{html}");
}
