use super::*;

fn view_link<'a>(sidebar: &'a str, id: &str) -> &'a str {
    let marker = format!("data-view=\"{id}\"");
    let start = sidebar
        .find(&marker)
        .unwrap_or_else(|| panic!("lien de la vue {id} absent"));
    let end = sidebar[start..]
        .find("</a>")
        .map_or(sidebar.len(), |end| start + end);
    &sidebar[start..end]
}

fn view_group_head<'a>(sidebar: &'a str, id: &str) -> &'a str {
    let marker = format!("data-view-group=\"{id}\"");
    let start = sidebar
        .find(&marker)
        .unwrap_or_else(|| panic!("en-tete du groupe de vues {id} absent"));
    let end = sidebar[start..]
        .find("</summary>")
        .map_or(sidebar.len(), |end| start + end);
    &sidebar[start..end]
}

#[tokio::test]
async fn sidebar_sets_and_clears_a_view_group_color() {
    let fixture = fixture().await;
    let group = fixture
        .app
        .create_view_group("Perso", Some(fs_ez_core::TagColor::Pink))
        .expect("groupe de vues");
    let id = group.id.0.to_string();

    let (_, sidebar) = fixture.page("/partials/sidebar").await;
    assert!(
        view_group_head(&sidebar, &id).contains("dot--pink"),
        "{sidebar}"
    );
    assert!(
        sidebar.contains(&format!(
            "hx-post=\"/partials/view-groups/{}/color\"",
            group.id
        )),
        "{sidebar}"
    );
    assert!(!sidebar.contains("dot--none"), "{sidebar}");

    let response = fixture
        .post_hx(
            &format!("/partials/view-groups/{}/color", group.id),
            "color=red",
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let html = body_text(response).await;
    assert!(view_group_head(&html, &id).contains("dot--red"), "{html}");
    assert_eq!(
        fixture.app.list_view_groups().expect("groupes")[0].color,
        Some(fs_ez_core::TagColor::Red)
    );

    let response = fixture
        .post_hx(
            &format!("/partials/view-groups/{}/color", group.id),
            "color=",
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let html = body_text(response).await;
    assert!(!view_group_head(&html, &id).contains("dot--"), "{html}");
    assert_eq!(
        fixture.app.list_view_groups().expect("groupes")[0].color,
        None
    );

    let (_, sidebar) = fixture.page("/partials/sidebar").await;
    assert!(
        !view_group_head(&sidebar, &id).contains("dot--"),
        "{sidebar}"
    );
}

#[tokio::test]
async fn sidebar_sets_and_clears_a_view_color_and_renders_its_dot() {
    let fixture = fixture().await;
    fixture
        .app
        .create_view_group("Divers", None)
        .expect("groupe de vues");
    let view = fixture
        .app
        .create_view(ViewInput {
            name: "Factures".to_string(),
            filter: Some(FilterExpr::Tag {
                tag: "facture".to_string(),
                negated: false,
            }),
            icon: None,
            color: Some(fs_ez_core::TagColor::Amber),
            position: None,
            sort: Sort::default(),
        })
        .expect("vue");
    let id = view.id.0.to_string();

    let (_, sidebar) = fixture.page("/partials/sidebar").await;
    assert!(view_link(&sidebar, &id).contains("dot--amber"), "{sidebar}");
    assert!(
        sidebar.contains(&format!("hx-post=\"/views/{}/color\"", view.id)),
        "{sidebar}"
    );
    assert!(!sidebar.contains("dot--none"), "{sidebar}");
    assert!(!sidebar.contains("title=\"No color\""), "{sidebar}");

    let response = fixture
        .post_hx(&format!("/views/{}/color", view.id), "color=blue")
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let html = body_text(response).await;
    assert!(view_link(&html, &id).contains("dot--blue"), "{html}");
    assert_eq!(
        fixture.view(view.id).color,
        Some(fs_ez_core::TagColor::Blue)
    );

    let response = fixture
        .post_hx(&format!("/views/{}/color", view.id), "color=")
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let html = body_text(response).await;
    assert!(!view_link(&html, &id).contains("dot--"), "{html}");
    assert_eq!(fixture.view(view.id).color, None);

    let (_, sidebar) = fixture.page("/partials/sidebar").await;
    assert!(!view_link(&sidebar, &id).contains("dot--"), "{sidebar}");
}

#[tokio::test]
async fn create_and_edit_forms_accept_and_preserve_view_color() {
    let fixture = fixture().await;
    fixture
        .app
        .create_view_group("Divers", None)
        .expect("groupe de vues");

    let body = form(&[
        ("name", "Coloree"),
        ("filter_json", ""),
        ("query", "facture"),
        ("sort", "id:asc"),
        ("color", "teal"),
    ]);
    let response = fixture.post_hx("/views", &body).await;
    assert_eq!(response.status(), StatusCode::OK);
    let created = fixture
        .app
        .list_views()
        .expect("vues")
        .into_iter()
        .find(|view| view.name == "Coloree")
        .expect("vue");
    assert_eq!(created.color, Some(fs_ez_core::TagColor::Teal));

    let body = form(&[
        ("name", "Coloree"),
        ("filter_json", ""),
        ("query", "facture"),
        ("sort", "name:asc"),
        ("color", ""),
    ]);
    let response = fixture
        .post_hx(&format!("/views/{}", created.id), &body)
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(fixture.view(created.id).color, None);
}

#[tokio::test]
async fn sidebar_exposes_create_edit_and_delete_controls() {
    let fixture = fixture().await;
    let view = fixture
        .app
        .create_view(ViewInput {
            name: "Factures".to_string(),
            filter: Some(FilterExpr::Tag {
                tag: "facture".to_string(),
                negated: false,
            }),
            icon: None,
            color: None,
            position: None,
            sort: Sort::default(),
        })
        .expect("vue");

    let (status, sidebar) = fixture.page("/partials/sidebar").await;
    assert_eq!(status, StatusCode::OK);
    assert!(sidebar.contains("data-view-create-open"), "{sidebar}");
    assert!(sidebar.contains("data-view-create-form"), "{sidebar}");
    assert!(sidebar.contains("action=\"/views\""), "{sidebar}");
    assert!(
        sidebar.contains(&format!("hx-get=\"/partials/views/{}/edit\"", view.id)),
        "{sidebar}"
    );
    assert!(sidebar.contains("data-view-rename-open"), "{sidebar}");
    assert!(
        sidebar.contains(&format!("hx-post=\"/views/{}/rename\"", view.id)),
        "{sidebar}"
    );
    assert!(
        sidebar.contains(&format!("hx-post=\"/views/{}/duplicate\"", view.id)),
        "{sidebar}"
    );
    assert!(
        sidebar.contains(&format!("hx-post=\"/views/{}/delete\"", view.id)),
        "{sidebar}"
    );

    let start = sidebar
        .find("data-view-create-form")
        .expect("formulaire de creation");
    let end = sidebar[start..]
        .find("</form>")
        .map_or(sidebar.len(), |end| start + end);
    let form = &sidebar[start..end];
    assert_eq!(form.matches("sidebar__tag-swatch").count(), 12, "{form}");
    assert_eq!(form.matches(" is-on").count(), 1, "{form}");
    assert!(form.contains("type=\"radio\" name=\"color\""), "{form}");
    assert!(!form.contains("dot--none"), "{form}");
}

#[tokio::test]
async fn legacy_view_pages_are_gone() {
    let fixture = fixture().await;
    let (status, _) = fixture.page("/views").await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    let (status, _) = fixture.page("/views/new").await;
    assert!(!status.is_success());
}

#[tokio::test]
async fn sidebar_puts_new_views_at_the_top_of_their_group() {
    let fixture = fixture().await;
    let filter = FilterExpr::Tag {
        tag: "facture".to_string(),
        negated: false,
    };
    let group = fixture
        .app
        .create_view_group("Perso", None)
        .expect("groupe");
    let make = |name: &str| {
        fixture
            .app
            .create_view_in_group(
                ViewInput {
                    name: name.to_string(),
                    filter: Some(filter.clone()),
                    icon: None,
                    color: None,
                    position: None,
                    sort: Sort::default(),
                },
                Some(group.id),
            )
            .expect("vue")
    };
    make("premiere");
    make("seconde");

    let (status, sidebar) = fixture.page("/partials/sidebar").await;
    assert_eq!(status, StatusCode::OK);
    let premiere = sidebar.find("premiere").expect("premiere");
    let seconde = sidebar.find("seconde").expect("seconde");
    assert!(seconde < premiere, "{sidebar}");
    assert!(!sidebar.contains("sidebar__count"), "{sidebar}");
}

#[tokio::test]
async fn rename_changes_only_the_view_name() {
    let fixture = fixture().await;
    let filter = FilterExpr::Tag {
        tag: "facture".to_string(),
        negated: false,
    };
    let view = fixture
        .app
        .create_view(ViewInput {
            name: "Avant".to_string(),
            filter: Some(filter.clone()),
            icon: None,
            color: None,
            position: None,
            sort: Sort::default(),
        })
        .expect("vue");

    let body = form(&[("name", "Après")]);
    let response = fixture
        .post_hx(&format!("/views/{}/rename", view.id), &body)
        .await;
    assert_eq!(response.status(), StatusCode::OK);

    let renamed = fixture.view(view.id);
    assert_eq!(renamed.name, "Après");
    assert_eq!(renamed.filter, filter);
    assert_eq!(renamed.sort, view.sort);
    assert_eq!(renamed.position, view.position);
}
