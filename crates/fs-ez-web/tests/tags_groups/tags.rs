use super::*;

#[tokio::test]
async fn create_blue_tag_is_visible_in_sidebar() {
    let fixture = fixture().await;
    let group = fixture.tag_group();
    let response = fixture
        .post_hx(
            "/partials/tags",
            &format!("name=facture&color=blue&group={group}"),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_fragment_response(&response);
    let body = body_text(response).await;
    assert!(body.contains("facture"), "{body}");
    assert!(body.contains("dot--blue"), "{body}");
    assert!(body.contains("hx-swap-oob=\"outerHTML\""), "{body}");
    assert!(body.contains("id=\"sidebar\""), "{body}");

    let tag = fixture
        .app
        .store
        .get_tag_by_name("facture")
        .expect("recherche")
        .expect("tag");
    assert_eq!(tag.color, TagColor::Blue);

    let (status, sidebar) = fixture.page("/partials/sidebar").await;
    assert_eq!(status, StatusCode::OK);
    assert!(sidebar.contains("facture"));
    assert!(!sidebar.contains("<html"));
}

#[tokio::test]
async fn invalid_color_and_empty_or_long_names_keep_data_unchanged() {
    let fixture = fixture().await;
    let group = fixture.tag_group();
    for body in [
        format!("name=invalide&color=magenta&group={group}"),
        format!("name=&color=blue&group={group}"),
        format!("name={}&color=blue&group={group}", "a".repeat(65)),
    ] {
        let response = fixture.post("/partials/tags", &body).await;
        assert_eq!(response.status(), StatusCode::OK);
        let html = body_text(response).await;
        assert!(html.contains("role=\"alert\""), "{html}");
        assert!(html.contains("hx-swap-oob=\"outerHTML\""), "{html}");
    }
    assert!(fixture.app.list_tags().expect("tags").is_empty());
}

#[tokio::test]
async fn duplicate_tag_name_is_reported_without_sql_text() {
    let fixture = fixture().await;
    let group = fixture.tag_group();
    let first = fixture
        .post_hx(
            "/partials/tags",
            &format!("name=facture&color=blue&group={group}"),
        )
        .await;
    assert_eq!(first.status(), StatusCode::OK);
    body_text(first).await;

    let response = fixture
        .post_hx(
            "/partials/tags",
            &format!("name=facture&color=green&group={group}"),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let html = body_text(response).await;
    assert!(
        html.contains("A tag with this name already exists."),
        "{html}"
    );
    assert!(!html.contains("UNIQUE"), "{html}");
    assert!(!html.contains("constraint"), "{html}");
    assert!(!html.contains("SQL"), "{html}");
    assert_eq!(fixture.app.list_tags().expect("tags").len(), 1);
}

#[tokio::test]
async fn renaming_a_tag_to_an_existing_name_is_reported_without_sql_text() {
    let fixture = fixture().await;
    fixture.create_tag("premier", TagColor::Blue);
    fixture.create_tag("second", TagColor::Green);
    let id = fixture.tag_id("second");

    let response = fixture
        .post_hx(&format!("/partials/tags/{id}/rename"), "name=premier")
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let html = body_text(response).await;
    assert!(
        html.contains("A tag with this name already exists."),
        "{html}"
    );
    assert!(!html.contains("UNIQUE"), "{html}");
    assert!(!html.contains("constraint"), "{html}");
    assert!(!html.contains("SQL"), "{html}");
    let tags = fixture.app.list_tags().expect("tags");
    assert_eq!(tags.len(), 2);
    assert!(tags.iter().any(|tag| tag.tag.name == "second"));
}

#[tokio::test]
async fn rename_tag_updates_files_and_sidebar() {
    let fixture = fixture().await;
    fixture.create_tag("ancien", TagColor::Green);
    let file = fixture.add_file("rapport.txt", &["ancien"]);
    let id = fixture.tag_id("ancien");

    let response = fixture
        .post_hx(&format!("/partials/tags/{id}/rename"), "name=nouveau")
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_fragment_response(&response);
    let html = body_text(response).await;
    assert!(html.contains("nouveau"), "{html}");
    assert!(html.contains("id=\"sidebar\""), "{html}");

    let tags = fixture.app.file_with_tags(file).expect("fichier").tags;
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].tag.name, "nouveau");
}

#[tokio::test]
async fn delete_tag_removes_it_from_files_and_groups() {
    let fixture = fixture().await;
    fixture.create_tag("obsolete", TagColor::Red);
    let file = fixture.add_file("vieux.txt", &["obsolete"]);
    let group = fixture.app.create_group("Archive", None).expect("groupe");
    fixture
        .app
        .assign_tag(group.id, "obsolete")
        .expect("assignation");
    let id = fixture.tag_id("obsolete");

    let response = fixture
        .post_hx(&format!("/partials/tags/{id}/delete"), "")
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_fragment_response(&response);
    assert!(
        fixture
            .app
            .file_with_tags(file)
            .expect("fichier")
            .tags
            .is_empty()
    );
    assert!(
        fixture
            .app
            .tags_in_group(group.id)
            .expect("tags du groupe")
            .is_empty()
    );
    assert!(fixture.app.list_tags().expect("tags").is_empty());
}
