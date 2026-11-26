//! Protection CSRF du tagging.

use super::*;

#[tokio::test]
async fn cross_site_tagging_is_forbidden() {
    let fixture = fixture().await;
    let id = fixture.add_file("un.txt", b"1", &[]);
    let token = login(&fixture.router).await;
    let request = with_site(
        post_form(
            "/partials/files/tags",
            &format!("ids={id}&tags=x&action=add"),
        ),
        "cross-site",
    );
    let response = send(&fixture.router, with_cookie(request, &token)).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(fixture.tag_names(id).is_empty());
}
