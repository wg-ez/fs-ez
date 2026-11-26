//! Retrait de tag et validation des entrées.

use super::*;

#[tokio::test]
async fn table_pill_removal_updates_only_the_row_tags() {
    let fixture = fixture().await;
    let id = fixture.add_file("facture.pdf", b"abc", &["ancien", "garde"]);
    let token = login(&fixture.router).await;

    let response = send(
        &fixture.router,
        with_cookie(
            with_hx(post_form(
                &format!("/partials/file/{id}/tags/remove"),
                "tags=ancien",
            )),
            &token,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("hx-trigger")
            .and_then(|value| value.to_str().ok()),
        Some("files-changed, tags-changed")
    );
    let html = body_text(response).await;
    assert!(html.contains("data-remove-tag"), "{html}");
    assert!(html.contains("garde"), "{html}");
    assert!(!html.contains("ancien"), "{html}");

    let tags = fixture.tag_names(id);
    assert!(!tags.contains(&"ancien".to_string()), "{tags:?}");
    assert!(tags.contains(&"garde".to_string()), "{tags:?}");
}

#[tokio::test]
async fn table_pill_removal_without_referer_falls_back_to_root() {
    let fixture = fixture().await;
    let id = fixture.add_file("note.txt", b"x", &["ancien"]);
    let token = login(&fixture.router).await;

    let response = send(
        &fixture.router,
        with_cookie(
            post_form(&format!("/partials/file/{id}/tags/remove"), "tags=ancien"),
            &token,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response
            .headers()
            .get(header::LOCATION)
            .and_then(|value| value.to_str().ok()),
        Some("/")
    );
    assert!(!fixture.tag_names(id).contains(&"ancien".to_string()));
}

#[tokio::test]
async fn invalid_tag_input_is_reported_without_adding_anything() {
    let fixture = fixture().await;
    let id = fixture.add_file("note.txt", b"x", &["garde"]);
    let token = login(&fixture.router).await;

    for tags in ["a,,b", "%3Dx"] {
        let body = format!("ids={id}&tags={tags}&action=add");
        let response = send(
            &fixture.router,
            with_cookie(with_hx(post_form("/partials/files/tags", &body)), &token),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK, "{tags}");
        let html = body_text(response).await;
        assert!(html.contains("role=\"alert\""), "{tags}");
        assert_eq!(fixture.tag_names(id), ["garde"], "{tags}");
    }
}
