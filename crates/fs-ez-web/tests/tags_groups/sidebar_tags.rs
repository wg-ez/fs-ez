use super::*;

#[tokio::test]
async fn sidebar_create_color_and_delete_use_existing_endpoints_and_oob_sidebar() {
    let fixture = fixture().await;
    let group = fixture.app.create_group("Cible", None).expect("groupe");
    let response = post_sidebar(
        &fixture,
        "/partials/tags",
        &format!("name=depuis-sidebar&color=green&group={}", group.id),
    )
    .await;
    assert_fragment_response(&response);
    let html = body_text(response).await;
    assert_sidebar_oob(&html);
    assert!(html.contains("depuis-sidebar"), "{html}");
    assert!(html.contains("dot--green"), "{html}");
    let id = fixture.tag_id("depuis-sidebar");

    let response = post_sidebar(
        &fixture,
        &format!("/partials/tags/{id}/color"),
        "color=amber",
    )
    .await;
    assert_fragment_response(&response);
    assert!(body_text(response).await.contains("dot--amber"));

    let response = post_sidebar(&fixture, &format!("/partials/tags/{id}/delete"), "").await;
    assert_fragment_response(&response);
    let html = body_text(response).await;
    assert!(!html.contains("depuis-sidebar"), "{html}");
    assert!(
        fixture
            .app
            .store
            .get_tag_by_name("depuis-sidebar")
            .expect("recherche")
            .is_none()
    );
}

#[tokio::test]
async fn sidebar_validation_error_is_rendered_in_the_oob_sidebar() {
    let fixture = fixture().await;
    let group = fixture.app.create_group("Cible", None).expect("groupe");

    let response = post_sidebar(
        &fixture,
        "/partials/tags",
        &format!("name=&color=blue&group={}", group.id),
    )
    .await;
    assert!(
        response.headers().get("hx-trigger").is_none(),
        "une erreur ne doit pas rafraîchir la sidebar"
    );
    let html = body_text(response).await;
    assert_sidebar_oob(&html);
    assert!(
        html.contains("class=\"alert sidebar__error\" role=\"alert\""),
        "{html}"
    );
}

#[tokio::test]
async fn sidebar_created_tag_stays_at_the_top_of_its_group() {
    let fixture = fixture().await;
    let group = fixture.app.create_group("Cible", None).expect("groupe");
    fixture.create_tag("ancien", TagColor::Neutral);
    fixture
        .app
        .assign_tag(group.id, "ancien")
        .expect("assignation");

    let response = post_sidebar(
        &fixture,
        "/partials/tags",
        &format!("name=nouveau&group={}", group.id),
    )
    .await;
    let html = body_text(response).await;
    assert!(html.contains("nouveau"), "{html}");

    let names: Vec<String> = fixture
        .app
        .tags_in_group(group.id)
        .expect("tags du groupe")
        .into_iter()
        .map(|tag| tag.name)
        .collect();
    assert_eq!(names, vec!["nouveau".to_string(), "ancien".to_string()]);
}

#[tokio::test]
async fn tag_creation_error_reopens_the_group_form_with_the_draft() {
    let fixture = fixture().await;
    let group = fixture.app.create_group("Cible", None).expect("groupe");
    fixture.create_tag("existant", TagColor::Neutral);

    let response = post_sidebar(
        &fixture,
        "/partials/tags",
        &format!("name=existant&group={}", group.id),
    )
    .await;
    assert!(response.headers().get("hx-trigger").is_none());
    let html = body_text(response).await;

    let input = format!(
        "id=\"sidebar-tag-create-{}\" name=\"name\" value=\"existant\"",
        group.id
    );
    let input_index = html
        .find(&input)
        .unwrap_or_else(|| panic!("{input} absent de {html}"));
    let form_start = html[..input_index]
        .rfind("<form")
        .unwrap_or_else(|| panic!("formulaire absent de {html}"));
    let form_tag = &html[form_start..input_index];
    assert!(
        form_tag.contains("data-group-tag-form>"),
        "le formulaire doit rester ouvert : {form_tag}"
    );
    assert!(
        html.contains("A tag with this name already exists."),
        "{html}"
    );
    assert_eq!(fixture.app.list_tags().expect("tags").len(), 1);
}

#[tokio::test]
async fn sidebar_creates_tag_inside_group_with_random_color() {
    let fixture = fixture().await;
    let group = fixture.app.create_group("Cible", None).expect("groupe");

    let (status, sidebar) = fixture.page("/partials/sidebar").await;
    assert_eq!(status, StatusCode::OK);
    let marker = "type=\"hidden\" name=\"color\" value=\"";
    let start = sidebar.find(marker).expect("couleur du placeholder") + marker.len();
    let token = sidebar[start..start + sidebar[start..].find('"').expect("jeton")].to_string();
    assert!(sidebar.contains(&format!("dot--{token}")), "{sidebar}");
    assert!(TagColor::parse(&token).is_some(), "{token}");

    let response = post_sidebar(
        &fixture,
        "/partials/tags",
        &format!("name=dans-groupe&color={token}&group={}", group.id),
    )
    .await;
    assert_fragment_response(&response);
    let html = body_text(response).await;
    assert!(html.contains("dans-groupe"), "{html}");

    let tags = fixture.app.tags_in_group(group.id).expect("tags du groupe");
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].name, "dans-groupe");
    assert_eq!(tags[0].color.as_token(), token);
    assert!(html.contains(&format!("dot--{token}")), "{html}");
}
