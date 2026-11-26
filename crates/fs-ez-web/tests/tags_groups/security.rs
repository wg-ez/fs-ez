use super::*;

#[tokio::test]
async fn cross_site_mutation_is_forbidden() {
    let fixture = fixture().await;
    let response = send(
        &router(&fixture.state),
        with_cookie(
            with_site(
                post_form("/partials/tags", "name=interdit&color=blue"),
                "cross-site",
            ),
            &fixture.cookie,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(fixture.app.list_tags().expect("tags").is_empty());
}
