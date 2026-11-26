use super::*;

#[tokio::test]
async fn bulk_edit_reports_partial_success() {
    let fixture = web_fixture().await;
    let first = fixture.add_file("un.txt", b"1", &[]);
    let second = fixture.add_file("deux.txt", b"2", &[]);
    let third = fixture.add_file("trois.txt", b"3", &[]);

    let body = format!("ids={first}&ids={second}&ids={third}&ids=9999&tags=un,deux&action=add");
    let response = fixture
        .send(with_cookie(
            with_hx(post_form("/partials/files/tags", &body)),
            &fixture.cookie,
        ))
        .await;
    assert_status(&response, StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("hx-trigger")
            .and_then(|value| value.to_str().ok()),
        Some("files-changed, tags-changed")
    );
    let html = body_text(response).await;
    assert!(html.contains("3 files updated"), "{html}");
    assert!(html.contains("1 failed"), "{html}");
    for id in [first, second, third] {
        let tags = fixture.tag_names(id);
        assert!(tags.contains(&"un".to_string()), "{id}");
        assert!(tags.contains(&"deux".to_string()), "{id}");
    }
}
