//! Fiche 30 : préférences de liste, thème, langue et sidebar masquée.

mod common;

use common::*;
use fs_ez_core::{Sort, SortDir, SortField};

async fn fixture() -> Fixture {
    Fixture::new(FixtureOptions::default().persistent_db()).await
}

trait PrefsExt {
    async fn get_lang(&self, uri: &str, language: &str) -> Response<Body>;
}

impl PrefsExt for Fixture {
    async fn get_lang(&self, uri: &str, language: &str) -> Response<Body> {
        let mut request = with_cookie(get(uri), &self.cookie);
        request.headers_mut().insert(
            header::ACCEPT_LANGUAGE,
            header::HeaderValue::from_str(language).expect("langue"),
        );
        send(&self.router(), request).await
    }
}

#[tokio::test]
async fn preferences_post_writes_and_survives_restart() {
    let fixture = fixture().await;
    let body = "recent_days=30&recent_file_days=14&page_size=50&sort=name:asc";
    let response = fixture.post("/settings/preferences", body).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&response), "/");

    let settings = fixture.reopened_settings();
    assert_eq!(settings.recent_days, 30);
    assert_eq!(settings.recent_file_days, 14);
    assert_eq!(settings.page_size, 50);
    assert_eq!(
        settings.default_sort,
        Sort {
            field: SortField::Name,
            dir: SortDir::Asc
        }
    );
}

#[tokio::test]
async fn preferences_htmx_save_returns_a_toast() {
    let fixture = fixture().await;
    let response = fixture
        .post_hx(
            "/settings/preferences",
            "recent_days=30&page_size=50&sort=name:asc",
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let toast = body_text(response).await;
    assert!(toast.contains("Settings saved."), "{toast}");
    assert_eq!(fixture.setting("ui.page_size").as_deref(), Some("50"));
}

#[tokio::test]
async fn invalid_preferences_are_refused_without_writing() {
    let fixture = fixture().await;
    let before = fixture.app.settings().expect("réglages");
    let body = "recent_days=0&page_size=5&sort=bogus";
    let response = fixture.post("/settings/preferences", body).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let after = fixture.app.settings().expect("réglages");
    assert_eq!(before, after);
}

#[tokio::test]
async fn settings_page_renders_current_values() {
    let fixture = fixture().await;
    let (status, page) = fixture.page("/settings").await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("action=\"/settings/preferences\""), "{page}");
    assert!(page.contains("hx-post=\"/settings/preferences\""), "{page}");
    assert!(page.contains("value=\"7\""), "{page}");
    assert!(page.contains("name=\"recent_file_days\""), "{page}");
    assert!(page.contains("value=\"100\""), "{page}");
    assert!(page.contains("name=\"backup_days\""), "{page}");
    assert!(page.contains("name=\"backup_hour\""), "{page}");
    assert!(page.contains("action=\"/settings/backup/run\""), "{page}");
    assert!(page.contains("form=\"backup-settings\""), "{page}");
    assert!(page.contains("hx-include=\"#backup-settings\""), "{page}");
    assert!(page.contains("Backup now"), "{page}");
    assert!(!page.contains("Back up now"), "{page}");
    assert!(!page.contains("Automatic database copies"), "{page}");
    assert!(page.contains("action=\"/partials/inbox/rescan\""), "{page}");
    assert!(page.contains("action=\"/settings/tokens\""), "{page}");
}

#[tokio::test]
async fn backup_settings_write_retention_and_hour() {
    let fixture = fixture().await;
    let response = fixture
        .post_hx("/settings/backup", "backup_days=30&backup_hour=5")
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let toast = body_text(response).await;
    assert!(toast.contains("Settings saved."), "{toast}");

    let settings = fixture.reopened_settings();
    assert_eq!(settings.backup_days, 30);
    assert_eq!(settings.backup_hour, 5);
    assert_eq!(fixture.setting("io.backup_days").as_deref(), Some("30"));
    assert_eq!(fixture.setting("io.backup_hour").as_deref(), Some("5"));
}

#[tokio::test]
async fn manual_backup_creates_a_copy() {
    let fixture = fixture().await;
    let response = fixture.post_hx("/settings/backup/run", "").await;
    assert_eq!(response.status(), StatusCode::OK);
    let toast = body_text(response).await;
    assert!(toast.contains("Backup created."), "{toast}");

    let backups: Vec<_> = std::fs::read_dir(fixture.app.backup_dir())
        .expect("dossier backup")
        .filter_map(Result::ok)
        .collect();
    assert_eq!(backups.len(), 1);
}

#[tokio::test]
async fn theme_query_sets_cookie_and_setting() {
    let fixture = fixture().await;
    let response = fixture.get("/?theme=dark").await;
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = header_string(&response, header::SET_COOKIE);
    assert!(cookie.contains("fs-ez_theme=dark"), "{cookie}");
    assert!(cookie.contains("Max-Age=31536000"), "{cookie}");
    assert!(cookie.contains("SameSite=Lax"), "{cookie}");
    assert!(!cookie.contains("HttpOnly"), "{cookie}");
    let html = body_text(response).await;
    assert!(html.contains("data-theme=\"dark\""));
    assert_eq!(fixture.setting("ui.theme").as_deref(), Some("dark"));
}

#[tokio::test]
async fn language_query_switches_labels_and_ignores_unknown() {
    let fixture = fixture().await;
    let response = fixture.get("/?lang=fr").await;
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = header_string(&response, header::SET_COOKIE);
    assert!(cookie.contains("fs-ez_lang=fr"), "{cookie}");
    let html = body_text(response).await;
    assert!(html.contains("lang=\"fr\""));
    assert!(html.contains("Récemment taggés"), "{html}");
    assert_eq!(fixture.setting("ui.language").as_deref(), Some("fr"));

    let unknown = fixture.get("/?lang=de").await;
    let cookie = header_string(&unknown, header::SET_COOKIE);
    assert!(!cookie.contains("fs-ez_lang"), "{cookie}");
    let html = body_text(unknown).await;
    assert!(html.contains("lang=\"en\""));
}

#[tokio::test]
async fn accept_language_selects_french_without_cookie() {
    let fixture = fixture().await;
    let response = fixture.get_lang("/", "fr-FR,fr;q=0.9").await;
    let html = body_text(response).await;
    assert!(html.contains("lang=\"fr\""), "{html}");
    assert!(html.contains("Tous les fichiers"), "{html}");
}

#[tokio::test]
async fn cookie_wins_over_accept_language() {
    let fixture = fixture().await;
    let mut request = with_cookie(get("/"), &fixture.cookie);
    request.headers_mut().insert(
        header::COOKIE,
        header::HeaderValue::from_str(&format!("fs-ez_session={}; fs-ez_lang=en", fixture.cookie))
            .expect("cookie"),
    );
    request.headers_mut().insert(
        header::ACCEPT_LANGUAGE,
        header::HeaderValue::from_static("fr"),
    );
    let response = send(&fixture.router(), request).await;
    let html = body_text(response).await;
    assert!(html.contains("lang=\"en\""), "{html}");
}

#[tokio::test]
async fn sidebar_toggle_persists_and_renders_oob() {
    let fixture = fixture().await;
    let response = fixture.post_hx("/partials/sidebar/toggle", "").await;
    assert_eq!(response.status(), StatusCode::OK);
    let html = body_text(response).await;
    assert!(html.contains("hx-swap-oob=\"outerHTML\""), "{html}");
    assert_eq!(
        fixture.setting("ui.sidebar_collapsed").as_deref(),
        Some("1")
    );

    let page = body_text(fixture.get("/").await).await;
    assert!(page.contains("shell shell--sidebar-hidden"), "{page}");

    let response = fixture.post_hx("/partials/sidebar/toggle", "").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        fixture.setting("ui.sidebar_collapsed").as_deref(),
        Some("0")
    );
    let page = body_text(fixture.get("/").await).await;
    assert!(!page.contains("shell--sidebar-hidden"));
}

#[tokio::test]
async fn sidebar_toggle_redirects_without_htmx() {
    let fixture = fixture().await;
    let response = fixture.post("/partials/sidebar/toggle", "").await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        fixture.setting("ui.sidebar_collapsed").as_deref(),
        Some("1")
    );
}
