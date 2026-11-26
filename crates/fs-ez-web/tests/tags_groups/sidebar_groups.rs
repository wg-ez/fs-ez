use super::*;

fn create_form<'a>(sidebar: &'a str, marker: &str) -> &'a str {
    let index = sidebar
        .find(marker)
        .unwrap_or_else(|| panic!("formulaire {marker} absent"));
    let start = sidebar[..index]
        .rfind("<form")
        .expect("ouverture du formulaire");
    let end = sidebar[index..]
        .find("</form>")
        .map_or(sidebar.len(), |end| index + end);
    &sidebar[start..end]
}

fn hidden_color(form: &str) -> &str {
    form.split("name=\"color\" value=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("champ couleur")
}

fn group_head<'a>(sidebar: &'a str, id: &str) -> &'a str {
    let marker = format!("data-group=\"{id}\"");
    let start = sidebar
        .find(&marker)
        .unwrap_or_else(|| panic!("en-tete du groupe {id} absent"));
    let end = sidebar[start..]
        .find("</summary>")
        .map_or(sidebar.len(), |end| start + end);
    &sidebar[start..end]
}

#[tokio::test]
async fn sidebar_sets_and_clears_a_group_color() {
    let fixture = fixture().await;
    let group = fixture.app.create_group("Cible", None).expect("groupe");

    let (status, sidebar) = fixture.page("/partials/sidebar").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        sidebar.contains(&format!("hx-post=\"/partials/groups/{}/color\"", group.id)),
        "{sidebar}"
    );
    assert!(!sidebar.contains("dot--none"), "{sidebar}");
    assert!(!sidebar.contains("title=\"No color\""), "{sidebar}");

    let response = post_sidebar(
        &fixture,
        &format!("/partials/groups/{}/color", group.id),
        "color=teal",
    )
    .await;
    assert_fragment_response(&response);
    let html = body_text(response).await;
    assert!(group_head(&html, &group.id.0.to_string()).contains("dot--teal"));
    assert_eq!(
        fixture.app.list_groups().expect("groupes")[0].color,
        Some(TagColor::Teal)
    );

    let response = post_sidebar(
        &fixture,
        &format!("/partials/groups/{}/color", group.id),
        "color=",
    )
    .await;
    assert_fragment_response(&response);
    let html = body_text(response).await;
    assert!(!group_head(&html, &group.id.0.to_string()).contains("dot--"));
    assert_eq!(fixture.app.list_groups().expect("groupes")[0].color, None);

    let (_, sidebar) = fixture.page("/partials/sidebar").await;
    assert!(!group_head(&sidebar, &group.id.0.to_string()).contains("dot--"));
}

#[tokio::test]
async fn sidebar_empty_group_name_is_rejected_without_creating_a_group() {
    let fixture = fixture().await;
    let response = post_sidebar(&fixture, "/partials/groups", "name=   ").await;
    let html = body_text(response).await;

    assert!(html.contains("Enter a group name."), "{html}");
    assert!(!html.contains("UNIQUE"), "{html}");
    assert!(!html.contains("constraint"), "{html}");
    assert!(fixture.app.list_groups().expect("groupes").is_empty());
}

#[tokio::test]
async fn sidebar_new_group_uses_the_submitted_name_and_previewed_color() {
    let fixture = fixture().await;
    let (_, sidebar) = fixture.page("/partials/sidebar").await;
    let form = create_form(&sidebar, "data-group-create-form");
    let color = hidden_color(form);
    assert!(TagColor::parse(color).is_some(), "{color}");
    assert!(
        form.contains(&format!("dot dot--{color} sidebar__tag-color")),
        "{form}"
    );

    let body = format!("name=Equipe&color={color}");
    let response = post_sidebar(&fixture, "/partials/groups", &body).await;
    assert_fragment_response(&response);
    let html = body_text(response).await;

    let groups = fixture.app.list_groups().expect("groupes");
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].name, "Equipe");
    assert_eq!(groups[0].color, TagColor::parse(color));
    assert!(html.contains("Equipe"), "{html}");
    assert!(!html.contains("is-renaming"), "{html}");
    assert!(!html.contains("autofocus"), "{html}");
    assert!(
        html.contains(&format!("data-group-row=\"{}\"", groups[0].id)),
        "{html}"
    );
}

#[tokio::test]
async fn duplicate_group_name_is_reported_without_sql_text() {
    let fixture = fixture().await;
    let first = post_sidebar(&fixture, "/partials/groups", "name=Equipe").await;
    body_text(first).await;

    let response = post_sidebar(&fixture, "/partials/groups", "name=Equipe").await;
    assert!(
        response.headers().get("hx-trigger").is_none(),
        "une erreur ne doit pas rafraîchir la sidebar"
    );
    let html = body_text(response).await;
    assert!(
        html.contains("A group with this name already exists."),
        "{html}"
    );
    assert_no_sql_leak(&html);
    assert_eq!(fixture.app.list_groups().expect("groupes").len(), 1);
}

#[tokio::test]
async fn renaming_a_group_to_an_existing_name_is_reported_without_sql_text() {
    let fixture = fixture().await;
    let first = fixture.app.create_group("Premier", None).expect("groupe");
    let second = fixture.app.create_group("Second", None).expect("groupe");

    let response = post_sidebar(
        &fixture,
        &format!("/partials/groups/{}/rename", second.id),
        "name=Premier",
    )
    .await;
    let html = body_text(response).await;
    assert!(
        html.contains("A group with this name already exists."),
        "{html}"
    );
    assert_no_sql_leak(&html);
    let names: Vec<String> = fixture
        .app
        .list_groups()
        .expect("groupes")
        .into_iter()
        .map(|group| group.name)
        .collect();
    assert_eq!(names, vec!["Second".to_string(), "Premier".to_string()]);
    assert_eq!(first.name, "Premier");
}
