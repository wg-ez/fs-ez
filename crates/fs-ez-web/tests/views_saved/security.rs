use super::*;

#[tokio::test]
async fn cross_site_view_mutation_is_forbidden() {
    let fixture = fixture().await;
    let response = send(
        &router(&fixture.state),
        with_cookie(
            with_site(
                post_form("/views", "name=interdite&query=facture"),
                "cross-site",
            ),
            &fixture.cookie,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(fixture.app.list_views().expect("vues").is_empty());
}
