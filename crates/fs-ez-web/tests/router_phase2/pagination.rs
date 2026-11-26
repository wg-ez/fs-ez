use super::*;

#[tokio::test]
async fn offset_chunks_with_page_size_two_have_no_duplicate() {
    let fixture = paged_fixture(2).await;
    for index in 1..=5 {
        fixture.add_file(&format!("page-{index}.txt"), b"x", &[]);
    }

    let first = "/partials/files/rows?sort=id%3Aasc&offset=0&limit=2";
    let (status, first_html) = fixture.page(first).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(row_count(&first_html), 2);

    let second = "/partials/files/rows?sort=id%3Aasc&offset=2&limit=2";
    let (status, second_html) = fixture.page(second).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(row_count(&second_html), 2);

    let first_ids = selected_ids(&first_html);
    let second_ids = selected_ids(&second_html);
    assert_eq!(first_ids.len(), 2);
    assert_eq!(second_ids.len(), 2);
    for id in &second_ids {
        assert!(!first_ids.contains(id), "doublon {id}");
    }
}

#[tokio::test]
async fn rows_fragment_supports_offset() {
    let fixture = paged_fixture(2).await;
    for index in 1..=5 {
        fixture.add_file(&format!("page-{index}.txt"), b"x", &[]);
    }

    let (status, first) = fixture.page("/?sort=id:asc").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(row_count(&first), 2);
    assert!(first.contains("data-total=\"5\""));

    let by_offset = fixture
        .page("/partials/files/rows?sort=id%3Aasc&offset=4&limit=2")
        .await;
    assert_eq!(by_offset.0, StatusCode::OK);
    assert_eq!(row_count(&by_offset.1), 1);
    assert!(!by_offset.1.contains("<table"));
    assert!(!by_offset.1.contains("files-sentinel"));
}
