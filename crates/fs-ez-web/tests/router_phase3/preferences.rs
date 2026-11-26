use super::*;
use fs_ez_core::Theme;
use fs_ez_web::state::Event;

#[tokio::test]
async fn preferences_persist_recent_days_page_size_theme_language_and_sidebar() {
    let fixture = seeded().await;
    let response = fixture
        .post(
            "/settings/preferences",
            "recent_days=30&page_size=50&sort=name:asc",
        )
        .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&response), "/");

    fixture.get("/?theme=dark").await;
    fixture.get("/?lang=fr").await;
    let response = fixture.post_hx("/partials/sidebar/toggle", "").await;
    assert_eq!(response.status(), StatusCode::OK);

    let settings = fixture.reopened_settings();
    assert_eq!(settings.recent_days, 30);
    assert_eq!(settings.page_size, 50);
    assert_eq!(settings.theme, Theme::Dark);
    assert_eq!(settings.language.as_token(), "fr");
    assert!(settings.sidebar_collapsed);
    assert_eq!(fixture.setting("ui.recent_days").as_deref(), Some("30"));
    assert_eq!(fixture.setting("ui.page_size").as_deref(), Some("50"));
    assert_eq!(fixture.setting("ui.theme").as_deref(), Some("dark"));
    assert_eq!(fixture.setting("ui.language").as_deref(), Some("fr"));
    assert_eq!(
        fixture.setting("ui.sidebar_collapsed").as_deref(),
        Some("1")
    );

    let (status, page) = fixture.page("/settings").await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("data-theme=\"dark\""), "{page}");
    assert!(page.contains("lang=\"fr\""), "{page}");
    assert!(page.contains("value=\"30\""), "{page}");
    assert!(page.contains("value=\"50\""), "{page}");
}

#[tokio::test]
async fn preferences_ignore_cross_site_get() {
    let fixture = seeded().await;

    let request = with_site(with_cookie(get("/?lang=fr"), &fixture.cookie), "cross-site");
    let response = fixture.send(request).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(header_string(&response, header::SET_COOKIE).is_empty());
    assert_eq!(fixture.setting("ui.language"), None);
    let (_, page) = fixture.page("/").await;
    assert!(page.contains("lang=\"en\""), "{page}");

    let request = with_site(
        with_cookie(get("/?theme=dark"), &fixture.cookie),
        "same-site",
    );
    let response = fixture.send(request).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(header_string(&response, header::SET_COOKIE).is_empty());
    assert_eq!(fixture.setting("ui.theme"), None);

    let request = with_site(
        with_cookie(get("/?lang=fr"), &fixture.cookie),
        "same-origin",
    );
    let response = fixture.send(request).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(fixture.setting("ui.language").as_deref(), Some("fr"));
}

#[tokio::test]
async fn rescan_button_starts_background_inbox_import() {
    let fixture = Fixture::new(
        FixtureOptions::default()
            .create_files_dir()
            .create_inbox_dir(),
    )
    .await;
    std::fs::write(fixture.inbox().join("depot.txt"), b"depot").expect("dépôt");
    // Abonnement avant la requête : ImportFinished signale la fin réelle du scan, source déjà retirée.
    let mut events = fixture.state.events.subscribe();

    let response = fixture.post_hx("/partials/inbox/rescan", "").await;
    assert_eq!(response.status(), StatusCode::OK);
    let toast = body_text(response).await;
    assert!(toast.contains("rescan"), "{toast}");

    let imported = tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            match events.recv().await {
                Ok(Event::ImportFinished { imported, .. }) => return imported,
                Ok(_) => {}
                Err(error) => panic!("événements d'import: {error}"),
            }
        }
    })
    .await
    .expect("fichier inbox non importé");
    assert_eq!(imported, 1);

    assert!(
        fixture
            .app
            .find_by_name("depot.txt")
            .expect("base")
            .is_some()
    );
    assert!(!fixture.inbox().join("depot.txt").exists());
}
