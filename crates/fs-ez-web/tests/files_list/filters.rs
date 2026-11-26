use super::*;

#[tokio::test]
async fn untagged_recent_and_tag_filters() {
    let fixture = fixture(None).await;
    let token = login(&fixture.router).await;

    let (status, untagged) = page(&fixture.router, &token, "/?filter=untagged").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(row_count(&untagged), 10);

    let (status, recent) = page(&fixture.router, &token, "/?filter=recent").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(row_count(&recent), 15);

    let (status, tagged) = page(&fixture.router, &token, "/?tag=facture").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(row_count(&tagged), 10);
}

#[tokio::test]
async fn untagged_page_keeps_rows_after_tag_changes() {
    let fixture = fixture(None).await;
    let token = login(&fixture.router).await;

    let (_, untagged) = page(&fixture.router, &token, "/?filter=untagged").await;
    assert!(untagged.contains("hx-trigger=\"data-changed from:body, inbox-changed from:body\""));
    assert!(!untagged.contains("files-changed from:body"));

    let (_, all) = page(&fixture.router, &token, "/").await;
    assert!(all.contains(
        "hx-trigger=\"data-changed from:body, inbox-changed from:body, files-changed from:body\""
    ));
}

#[tokio::test]
async fn recent_files_page_uses_the_mtime_window() {
    let fixture = fixture(None).await;
    let token = login(&fixture.router).await;

    let (status, before) = page(&fixture.router, &token, "/?filter=recent-files").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(row_count(&before), 0);

    let now = OffsetDateTime::now_utc();
    let path = fixture.root().join("frais.txt");
    fixture
        .app
        .store
        .insert_file(&NewFile {
            path: path.clone(),
            path_display: path.to_string_lossy().into_owned(),
            name: "frais.txt".to_string(),
            dir: fixture.root().to_string_lossy().into_owned(),
            size: 1,
            mtime: now,
            kind: FileKind::File,
            fingerprint: None,
        })
        .expect("fichier récent");

    let (status, after) = page(&fixture.router, &token, "/?filter=recent-files").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(row_count(&after), 1);
    assert!(after.contains("frais.txt"), "{after}");
}

#[tokio::test]
async fn advanced_filter_combines_with_tags_and_saved_views() {
    use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};

    let fixture = fixture(None).await;
    let token = login(&fixture.router).await;
    let filter = serde_json::json!({
        "op": "and",
        "items": [
            { "op": "tag", "tag": "facture", "negated": false },
            { "op": "tag", "tag": "archive", "negated": true }
        ]
    })
    .to_string();
    let encoded = utf8_percent_encode(&filter, NON_ALPHANUMERIC).to_string();

    let (status, html) = page(&fixture.router, &token, &format!("/?filter_json={encoded}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(row_count(&html), 10);

    let (status, html) = page(
        &fixture.router,
        &token,
        &format!(
            "/views/{}?filter_json={encoded}",
            fixture.view.expect("vue")
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(row_count(&html), 10);
}
