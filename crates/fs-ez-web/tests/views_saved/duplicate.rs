use super::*;

#[tokio::test]
async fn duplicate_copies_filter_and_appends_position() {
    let fixture = fixture().await;
    let view = fixture
        .app
        .create_view(ViewInput {
            name: "Originale".to_string(),
            filter: Some(FilterExpr::Tag {
                tag: "facture".to_string(),
                negated: false,
            }),
            icon: None,
            color: None,
            position: None,
            sort: Sort::default(),
        })
        .expect("vue");

    let response = fixture
        .post_hx(&format!("/views/{}/duplicate", view.id), "")
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let views = fixture.app.list_views().expect("vues");
    assert_eq!(views.len(), 2);
    let copy = views
        .iter()
        .find(|candidate| candidate.id != view.id)
        .expect("copie");
    assert!(copy.name.contains("copy"), "{}", copy.name);
    assert_eq!(copy.filter, view.filter);
    assert!(copy.position > view.position);
}
