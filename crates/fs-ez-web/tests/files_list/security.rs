use super::*;

#[tokio::test]
async fn responses_never_offer_content_disposition_range_or_download() {
    let fixture = fixture(None).await;
    let token = login(&fixture.router).await;
    for uri in [
        "/",
        "/partials/files",
        &format!("/views/{}", fixture.view.expect("vue")),
    ] {
        let response = send(&fixture.router, with_cookie(get(uri), &token)).await;
        assert_eq!(response.status(), StatusCode::OK, "{uri}");
        assert!(
            response
                .headers()
                .get(header::CONTENT_DISPOSITION)
                .is_none()
        );
        assert!(response.headers().get(header::RANGE).is_none());
        let html = body_text(response).await;
        assert!(!html.to_ascii_lowercase().contains("download"), "{uri}");
        assert!(!html.contains("Content-Disposition"), "{uri}");
    }
}

#[tokio::test]
async fn non_utf8_path_never_fails_rendering() {
    let fixture = fixture(None).await;
    let token = login(&fixture.router).await;
    let raw = PathBuf::from(std::ffi::OsString::from_vec(vec![
        b'/', b't', b'm', b'p', b'/', 0xff,
    ]));
    let file = fixture
        .app
        .store
        .insert_file(&NewFile {
            path: raw,
            path_display: "/tmp/\u{fffd}".to_string(),
            name: "\u{fffd}".to_string(),
            dir: "/tmp".to_string(),
            size: 0,
            mtime: OffsetDateTime::UNIX_EPOCH,
            kind: FileKind::File,
            fingerprint: None,
        })
        .expect("fichier non-UTF8");
    let (status, html) = page(&fixture.router, &token, "/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains('\u{fffd}'));
    assert!(selected_ids(&html).contains(&file.id.to_string()));
}
