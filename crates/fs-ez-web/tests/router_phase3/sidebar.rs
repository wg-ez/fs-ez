use super::*;

#[tokio::test]
async fn sidebar_omits_file_counts_and_keeps_single_membership() {
    let fixture = seeded().await;
    let (status, sidebar) = fixture.page("/partials/sidebar").await;
    assert_eq!(status, StatusCode::OK);

    assert!(!sidebar.contains("sidebar__count"), "{sidebar}");

    let projets = sidebar.find("Projets").expect("vues epinglees");
    let factures = sidebar.find("Factures").expect("vues epinglees");
    assert!(projets < factures, "{sidebar}");

    let finance = sidebar.find("Finance").expect("groupe");
    assert!(sidebar[finance..].contains("data-tag=\"c-blue\""));
    let finance_head = &sidebar[finance..];
    let head_end = finance_head.find("</summary>").expect("entete");
    assert!(!finance_head[..head_end].contains("sidebar__count"));
    assert_eq!(sidebar.matches("data-tag=\"c-blue\"").count(), 1);

    assert!(!sidebar.contains("Ungrouped"), "{sidebar}");
    for tag in ["c-neutral", "c-orange", "c-cyan", "c-pink", "c-brown"] {
        assert!(sidebar.contains(&format!("data-tag=\"{tag}\"")), "{tag}");
    }
}

#[tokio::test]
async fn sidebar_tag_link_keeps_current_sort() {
    let fixture = seeded().await;
    let (status, page) = fixture.page("/?sort=name:desc").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        page.contains("href=\"/?tag=c-blue&#38;sort=name%3Adesc\""),
        "{page}"
    );
}

#[tokio::test]
async fn sidebar_marks_the_current_view_and_tag_as_active() {
    let fixture = seeded().await;
    let view = fixture
        .app
        .list_views()
        .expect("vues")
        .into_iter()
        .find(|view| view.name == "Factures")
        .expect("vue Factures");

    let (status, page) = fixture.page(&format!("/views/{}", view.id)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        page.contains(&format!(
            "class=\"sidebar__row sidebar__row--amber is-on\" data-view-row data-view-id=\"{}\"",
            view.id
        )),
        "{page}"
    );

    let (status, page) = fixture.page("/?tag=c-blue").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        page.contains("class=\"sidebar__row sidebar__row--blue is-on\" data-tag-row"),
        "{page}"
    );
}

#[tokio::test]
async fn sidebar_partial_keeps_virtual_folders_active_from_hx_url() {
    let fixture = seeded().await;
    let mut request = with_hx(with_cookie(get("/partials/sidebar"), &fixture.cookie));
    request.headers_mut().insert(
        "hx-current-url",
        header::HeaderValue::from_static("http://localhost/virtual-folders/4"),
    );
    let response = fixture.send(request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_text(response).await;
    assert!(
        body.contains("class=\"sidebar__link is-on\" href=\"/virtual-folders\""),
        "{body}"
    );
}

#[tokio::test]
async fn topbar_toggle_carries_labels_and_replaces_the_removed_footer_button() {
    let fixture = seeded().await;
    let (status, page) = fixture.page("/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        page.contains("class=\"icon-btn topbar__toggle js-rail-toggle\""),
        "{page}"
    );
    assert!(page.contains("aria-controls=\"sidebar\""), "{page}");
    assert!(page.contains("aria-expanded=\"true\""), "{page}");
    assert!(page.contains("title=\"Hide sidebar\""), "{page}");
    assert!(page.contains("aria-label=\"Hide sidebar\""), "{page}");
    assert!(
        page.contains("data-label-expand=\"Show sidebar\""),
        "{page}"
    );
    assert!(!page.contains("class=\"sidebar__toggle\""), "{page}");

    let response = fixture.post_hx("/partials/sidebar/toggle", "").await;
    assert_eq!(response.status(), StatusCode::OK);
    let (status, page) = fixture.page("/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("aria-expanded=\"false\""), "{page}");
    assert!(page.contains("title=\"Show sidebar\""), "{page}");
    assert!(page.contains("aria-label=\"Show sidebar\""), "{page}");
    assert!(
        page.contains("data-label-collapse=\"Hide sidebar\""),
        "{page}"
    );
}

#[tokio::test]
async fn hidden_sidebar_keeps_labels_title_aria_and_persists() {
    let fixture = seeded().await;
    let response = fixture.post_hx("/partials/sidebar/toggle", "").await;
    assert_eq!(response.status(), StatusCode::OK);
    let oob = body_text(response).await;
    assert!(oob.contains("hx-swap-oob=\"outerHTML\""), "{oob}");
    assert!(!oob.contains("sidebar__toggle"), "{oob}");

    let (status, page) = fixture.page("/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("shell shell--sidebar-hidden"), "{page}");
    assert!(page.contains("title=\"c-blue\""), "{page}");
    assert!(page.contains("aria-label=\"c-blue\""), "{page}");
    assert!(page.contains("title=\"Factures\""), "{page}");
    assert!(page.contains("aria-label=\"Factures\""), "{page}");
    assert_eq!(
        fixture.setting("ui.sidebar_collapsed").as_deref(),
        Some("1")
    );
    assert!(fixture.reopened_settings().sidebar_collapsed);

    let response = fixture.post("/partials/sidebar/toggle", "").await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let page = body_text(fixture.get("/").await).await;
    assert!(!page.contains("shell--sidebar-hidden"), "{page}");
    assert_eq!(
        fixture.setting("ui.sidebar_collapsed").as_deref(),
        Some("0")
    );
}
