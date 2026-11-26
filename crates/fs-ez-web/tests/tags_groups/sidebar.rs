use super::*;

#[tokio::test]
async fn sidebar_edits_name_and_color_from_row_menu() {
    let fixture = fixture().await;
    let group = fixture.app.create_group("Groupe", None).expect("groupe");
    let grouped = fixture.create_tag("groupe", TagColor::Blue);
    let ungrouped = fixture.create_tag("libre", TagColor::Pink);

    let (status, sidebar) = fixture.page("/partials/sidebar").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        sidebar.contains("class=\"sidebar__control sidebar__action\""),
        "{sidebar}"
    );
    assert!(sidebar.contains("sidebar__group-head"), "{sidebar}");
    assert!(sidebar.contains("data-group-menu"), "{sidebar}");
    assert!(sidebar.contains("data-group-drag"), "{sidebar}");
    assert!(
        sidebar.contains("hx-post=\"/partials/groups\""),
        "{sidebar}"
    );
    assert!(sidebar.contains("data-group-tag-open"), "{sidebar}");
    assert!(sidebar.contains("data-group-rename"), "{sidebar}");
    assert!(sidebar.contains("data-group-menu"), "{sidebar}");
    assert!(sidebar.contains("data-group-edit-open"), "{sidebar}");
    assert!(sidebar.contains("data-group-create"), "{sidebar}");
    assert!(sidebar.contains("data-group-create-form"), "{sidebar}");
    assert!(sidebar.contains("hx-target=\"#sidebar\""), "{sidebar}");
    assert!(sidebar.contains("hx-swap=\"none\""), "{sidebar}");
    assert!(
        sidebar.contains(&format!("action=\"/partials/groups/{}/delete\"", group.id)),
        "{sidebar}"
    );
    assert!(
        sidebar.contains("Delete this group and all its tags? Files will lose these tags."),
        "{sidebar}"
    );
    assert!(sidebar.contains("data-tag-menu"), "{sidebar}");
    assert!(sidebar.contains("data-tag-edit-open=\"name\""), "{sidebar}");
    assert!(
        sidebar.contains("data-tag-edit-open=\"color\""),
        "{sidebar}"
    );
    for id in [grouped.id, ungrouped.id] {
        assert!(
            sidebar.contains(&format!("action=\"/partials/tags/{id}/rename\"")),
            "{sidebar}"
        );
        assert!(
            sidebar.contains(&format!("action=\"/partials/tags/{id}/color\"")),
            "{sidebar}"
        );
        assert!(
            sidebar.contains(&format!("action=\"/partials/tags/{id}/delete\"")),
            "{sidebar}"
        );
        assert!(
            sidebar.contains(&format!("id=\"sidebar-tag-name-{id}\"")),
            "{sidebar}"
        );
    }
    assert!(
        sidebar.contains(&format!("data-group-list=\"{}\"", group.id)),
        "{sidebar}"
    );
    assert!(
        sidebar.contains(&format!("name=\"group\" value=\"{}\"", group.id)),
        "{sidebar}"
    );
    assert!(sidebar.contains("data-tag-edit=\"name\""), "{sidebar}");
    assert!(sidebar.contains("data-tag-color"), "{sidebar}");
    assert!(sidebar.contains("data-tag-name"), "{sidebar}");
    assert!(sidebar.contains("dot--blue"), "{sidebar}");
    assert!(sidebar.contains("dot--pink"), "{sidebar}");
    assert!(
        sidebar.contains("name=\"color\" value=\"blue\""),
        "{sidebar}"
    );
    assert!(
        sidebar.contains("name=\"color\" value=\"pink\""),
        "{sidebar}"
    );
}
#[tokio::test]
async fn every_mutation_response_contains_sidebar_oob() {
    let fixture = fixture().await;
    let group = fixture.app.create_group("Groupe", None).expect("groupe");
    fixture.create_tag("cible", TagColor::Violet);
    let tag_id = fixture.tag_id("cible");

    let bodies = [
        fixture
            .post_hx(
                "/partials/tags",
                &format!("name=autre&color=pink&group={}", group.id),
            )
            .await,
        fixture
            .post_hx(&format!("/partials/tags/{tag_id}/rename"), "name=cible2")
            .await,
        fixture
            .post_hx(&format!("/partials/tags/{tag_id}/color"), "color=green")
            .await,
        fixture
            .post_hx(&format!("/partials/groups/{}/rename", group.id), "name=G2")
            .await,
    ];
    for response in bodies {
        assert_eq!(response.status(), StatusCode::OK);
        assert_fragment_response(&response);
        let html = body_text(response).await;
        assert!(
            html.contains("<aside class=\"sidebar\" id=\"sidebar\""),
            "{html}"
        );
        assert_sidebar_oob(&html);
    }

    let response = fixture
        .post_hx(&format!("/partials/tags/{tag_id}/delete"), "")
        .await;
    assert_fragment_response(&response);
    assert!(body_text(response).await.contains("id=\"sidebar\""));
}
