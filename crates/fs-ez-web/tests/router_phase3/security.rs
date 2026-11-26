use super::*;
use fs_ez_core::Theme;

#[tokio::test]
async fn cross_site_mutations_are_forbidden_and_pages_stay_inline_free() {
    let fixture = seeded().await;
    let view = fixture.view_id("Factures");
    let tag = fixture.tag_id("c-blue");
    let group = fixture.group_id("Finance");
    let view_group = fixture.app.list_view_groups().expect("groupes de vues")[0].id;
    let attempts = [
        (
            "/partials/tags".to_string(),
            "name=interdit&color=blue&group=1".to_string(),
        ),
        (
            format!("/partials/tags/{tag}/rename"),
            "name=interdit".to_string(),
        ),
        (format!("/partials/groups/{group}/delete"), String::new()),
        (
            format!("/partials/groups/{group}/color"),
            "color=red".to_string(),
        ),
        (
            format!("/partials/view-groups/{view_group}/color"),
            "color=red".to_string(),
        ),
        (format!("/views/{view}/color"), "color=red".to_string()),
        (
            "/partials/tags/place".to_string(),
            format!("group={group}&tag=c-blue"),
        ),
        (
            "/views".to_string(),
            "name=interdite&query=facture".to_string(),
        ),
        (format!("/views/{view}/delete"), String::new()),
        (
            "/settings/preferences".to_string(),
            "recent_days=1&page_size=5&sort=name:asc".to_string(),
        ),
        ("/partials/sidebar/toggle".to_string(), String::new()),
    ];
    for (uri, body) in attempts {
        let response = send(
            &fixture.router(),
            with_cookie(
                with_site(post_form(&uri, &body), "cross-site"),
                &fixture.cookie,
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{uri}");
    }
    assert!(
        fixture
            .app
            .store
            .get_tag_by_name("interdit")
            .expect("recherche")
            .is_none()
    );
    assert_eq!(fixture.app.list_views().expect("vues").len(), 4);
    assert_eq!(fixture.reopened_settings().theme, Theme::Light);

    let (status, page) = fixture.page("/").await;
    assert_eq!(status, StatusCode::OK, "/");
    assert_no_inline_code(&page, "/");

    let file = fixture.file_id("facture-due.txt");
    for uri in [
        format!("/files/{file}/content"),
        format!("/download/{file}"),
        format!("/media/{file}"),
    ] {
        let (status, _) = fixture.page(&uri).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
    }
}
