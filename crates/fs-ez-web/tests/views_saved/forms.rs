use super::*;

#[tokio::test]
async fn sidebar_creates_views_and_edit_form_renders_the_builder() {
    let fixture = fixture().await;
    fixture.add_file("a.txt", &["facture"]);
    fixture
        .app
        .create_view_group("Divers", None)
        .expect("groupe de vues");
    let (status, sidebar) = fixture.page("/partials/sidebar").await;
    assert_eq!(status, StatusCode::OK);
    assert!(sidebar.contains("data-view-create-open"), "{sidebar}");
    assert!(sidebar.contains("data-view-create-form"), "{sidebar}");
    assert!(sidebar.contains("action=\"/views\""), "{sidebar}");
    assert!(
        sidebar.contains("id=\"view-sort-create\" name=\"sort\" data-dropdown"),
        "{sidebar}"
    );

    let view = fixture
        .app
        .create_view(ViewInput {
            name: "Facture".to_string(),
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
    let (status, editor) = fixture
        .page(&format!("/partials/views/{}/edit", view.id))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        editor.contains(&format!(
            "id=\"view-sort-{}\" name=\"sort\" data-dropdown",
            view.id
        )),
        "{editor}"
    );
    assert!(
        editor.contains(&format!("hx-post=\"/views/{}\"", view.id)),
        "{editor}"
    );
    assert!(
        editor.contains(&format!("id=\"view-filter-{}\"", view.id)),
        "{editor}"
    );
    assert!(
        editor.contains(&format!("data-view-editor data-view-id=\"{}\"", view.id)),
        "{editor}"
    );
    assert!(
        !editor.contains(&format!("id=\"view-name-{}\"", view.id)),
        "{editor}"
    );
    let row = editor
        .find(&format!("data-view-row data-view-id=\"{}\"", view.id))
        .expect("ligne de vue");
    let form = editor
        .find(&format!("data-view-editor data-view-id=\"{}\"", view.id))
        .expect("éditeur de vue");
    assert!(row < form, "l'éditeur doit rester sous sa ligne de vue");
    assert!(
        !editor.contains(&format!("data-view-id=\"{}\" hidden", view.id)),
        "{editor}"
    );
    for value in ["has", "has_not", "only", "not_only"] {
        assert!(editor.contains(&format!("value=\"{value}\"")), "{editor}");
    }
    assert!(editor.contains("builder__tagset-panel"), "{editor}");
    assert!(editor.contains("data-tag-option"), "{editor}");
    assert!(
        editor.contains("class=\"builder__tagset-chips\""),
        "{editor}"
    );
}
