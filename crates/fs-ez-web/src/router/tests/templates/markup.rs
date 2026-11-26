use super::*;

#[tokio::test]
async fn root_shell_is_script_free_and_uses_static_assets_only() {
    let temp = TempDir::new().expect("tempdir");
    let router = build_router(state(&temp));
    let response = login(&router, None).await;
    let token = session_token(&response);
    let response = send(&router, with_cookie(get("/"), &token)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let html = body_text(response).await;

    assert!(html.contains("fs-ez"));
    assert!(html.contains("class=\"sidebar"));
    assert!(html.contains("class=\"no-js\""));
    assert!(html.contains("id=\"work\""));
    assert!(!html.contains("<style"));
    assert!(!html.contains("style="));
    assert!(!html.contains("onclick="));
    assert!(!html.contains("<script>"));

    let mut rest = html.as_str();
    while let Some(position) = rest.find("src=\"") {
        let after = &rest[position + 5..];
        assert!(after.starts_with("/static/"), "{after}");
        rest = after;
    }
    let mut rest = html.as_str();
    while let Some(position) = rest.find("<script") {
        let after = &rest[position + 7..];
        assert!(after.starts_with(" src=\"/static/"), "{after}");
        rest = after;
    }

    for (open, close) in [
        ("<html", "</html>"),
        ("<body", "</body>"),
        ("<header", "</header>"),
        ("<nav", "</nav>"),
        ("<main", "</main>"),
        ("<aside", "</aside>"),
        ("<details", "</details>"),
    ] {
        assert_eq!(
            html.matches(open).count(),
            html.matches(close).count(),
            "{open}"
        );
    }
}

#[test]
fn files_page_refreshes_when_the_inbox_changes() {
    let template = static_source("templates/partials/files_page.html");
    assert!(template.contains("inbox-changed from:body"));
}

#[test]
fn templates_have_no_inline_style_or_script() {
    let templates = [
        "templates/layout.html",
        "templates/index.html",
        "templates/error.html",
        "templates/login.html",
        "templates/partials/chrome.html",
        "templates/partials/search_filter.html",
        "templates/partials/search_filter_panel.html",
        "templates/partials/sidebar.html",
        "templates/partials/ui.html",
        "templates/partials/files.html",
        "templates/partials/files_page.html",
        "templates/partials/file_rows.html",
        "templates/partials/folder_rows.html",
        "templates/partials/folder_static_rows.html",
        "templates/partials/toast.html",
    ];
    for template in templates {
        let source = static_source(template);
        assert!(!source.contains("style="), "{template}");
        assert!(!source.contains("<script>"), "{template}");
        assert!(!source.contains("<style"), "{template}");
    }
}

#[test]
fn static_assets_are_versioned_for_cache_busting() {
    let head = static_source("templates/partials/head.html");
    assert!(head.contains("/static/css/app.css?v={{ head_asset_version }}"));
    assert!(head.contains("/static/js/app/scroll.js"));
    assert!(head.contains("?v={{ head_asset_version }}"));
}
