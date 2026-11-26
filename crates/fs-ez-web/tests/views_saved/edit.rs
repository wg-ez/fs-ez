use super::*;

#[tokio::test]
async fn edit_updates_filter_and_sort_without_touching_metadata() {
    let fixture = fixture().await;
    fixture.add_file("alpha.txt", &["projet"]);
    let response = create_view(&fixture, "Projet", "", "projet").await;
    assert_eq!(response.status(), StatusCode::OK);
    let id = fixture.app.list_views().expect("vues")[0].id;
    let created = fixture.view(id);
    fixture
        .app
        .update_view(
            id,
            ViewInput {
                name: created.name,
                filter: Some(created.filter),
                icon: Some("P".to_string()),
                color: Some(fs_ez_core::TagColor::Blue),
                position: None,
                sort: created.sort,
            },
        )
        .expect("métadonnées");

    let filter = serde_json::to_string(&FilterExpr::Tag {
        tag: "projet".to_string(),
        negated: false,
    })
    .expect("json");
    let body = form(&[
        ("name", "Projet modifié"),
        ("filter_json", &filter),
        ("query", ""),
        ("sort", "name:desc"),
        ("color", "blue"),
        ("icon", "P"),
    ]);
    let response = fixture.post_hx(&format!("/views/{id}"), &body).await;
    assert_eq!(response.status(), StatusCode::OK);

    let view = fixture.view(id);
    assert_eq!(view.name, "Projet modifié");
    assert_eq!(view.sort.field, SortField::Name);
    assert_eq!(view.sort.dir, SortDir::Desc);
    assert_eq!(view.icon.as_deref(), Some("P"));
    assert_eq!(view.color, Some(fs_ez_core::TagColor::Blue));
    assert_eq!(
        view.filter,
        FilterExpr::Tag {
            tag: "projet".to_string(),
            negated: false,
        }
    );
}

#[tokio::test]
async fn edit_without_name_preserves_the_current_name() {
    let fixture = fixture().await;
    let view = fixture
        .app
        .create_view(ViewInput {
            name: "Intacte".to_string(),
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

    let body = form(&[
        ("filter_json", ""),
        ("query", "facture"),
        ("sort", "name:desc"),
    ]);
    let response = fixture.post_hx(&format!("/views/{}", view.id), &body).await;
    assert_eq!(response.status(), StatusCode::OK);

    let edited = fixture.view(view.id);
    assert_eq!(edited.name, "Intacte");
    assert_eq!(edited.sort.field, SortField::Name);
    assert_eq!(edited.sort.dir, SortDir::Desc);
}

#[tokio::test]
async fn delete_then_result_page_is_not_found() {
    let fixture = fixture().await;
    let view = fixture
        .app
        .create_view(ViewInput {
            name: "Temporaire".to_string(),
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
        .post_hx(&format!("/views/{}/delete", view.id), "")
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let (status, _) = fixture.page(&format!("/views/{}", view.id)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
