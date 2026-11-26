use super::*;

#[tokio::test]
async fn root_lists_files_with_headers_and_bounded_rows() {
    let fixture = fixture(None).await;
    let token = login(&fixture.router).await;
    let (status, html) = page(&fixture.router, &token, "/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("data-col-resizable"));
    assert!(html.contains("data-files-table"));
    for header in ["Name", "Tags", "Size", "Date"] {
        assert!(html.contains(&format!(">{header}<")), "{header}");
    }
    assert_eq!(html.matches("class=\"table__resizer\"").count(), 4);
    assert_eq!(row_count(&html), 25);
    assert!(row_count(&html) <= 100);
    assert!(html.contains("facture-01.txt"));
}

#[tokio::test]
async fn rows_expose_file_drop_targets_and_the_list_zone() {
    let fixture = fixture(None).await;
    let token = login(&fixture.router).await;
    let (status, html) = page(&fixture.router, &token, "/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("data-file-drop-zone"), "{html}");
    assert_eq!(
        html.matches("data-file-drop data-file-id=\"").count(),
        25,
        "{html}"
    );
    for id in selected_ids(&html) {
        assert!(
            html.contains(&format!("data-file-drop data-file-id=\"{id}\"")),
            "{id}"
        );
    }
}

#[tokio::test]
async fn sort_name_desc_orders_rows_descending() {
    let fixture = fixture(None).await;
    let token = login(&fixture.router).await;
    let (status, html) = page(&fixture.router, &token, "/?sort=name:desc").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        html.contains(
            "<th data-col=\"name\" aria-sort=\"descending\"><a class=\"table__sort is-on\""
        ),
        "{html}"
    );
    assert!(html.contains("sort=name%3Aasc"), "{html}");
    assert!(html.contains("hx-get=\"/?sort=name%3Aasc\""), "{html}");
    assert!(html.contains("hx-target=\"#work\""), "{html}");
    assert!(html.contains("hx-push-url=\"true\""), "{html}");
    let first = html.find("facture-24.txt").expect("f24");
    let rest = html.find("facture-01.txt").expect("f01");
    let rapport = html.find("Rapport-annuel.txt").expect("rapport");
    assert!(first < rest, "{first} < {rest}");
    assert!(rest < rapport, "{rest} < {rapport}");
}

#[tokio::test]
async fn search_matches_names_ignoring_case() {
    let fixture = fixture(None).await;
    let token = login(&fixture.router).await;
    let (status, html) = page(&fixture.router, &token, "/?q=rapport").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(row_count(&html), 1);
    assert!(html.contains("Rapport-annuel.txt"));
    assert!(!html.contains("facture-01.txt"));
}

#[tokio::test]
async fn search_form_preserves_sort_and_filter_but_not_offset() {
    let fixture = fixture(None).await;
    let token = login(&fixture.router).await;
    let (status, html) = page(&fixture.router, &token, "/?sort=name:desc&filter=untagged").await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("name=\"sort\" value=\"name:desc\""), "{html}");
    assert!(
        html.contains("name=\"filter\" value=\"untagged\""),
        "{html}"
    );
    assert!(!html.contains("name=\"offset\""), "{html}");
}
