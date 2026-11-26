use super::*;

#[tokio::test]
async fn saved_view_applies_its_filter_and_sort() {
    let fixture = fixture(None).await;
    let token = login(&fixture.router).await;
    let (status, html) = page(
        &fixture.router,
        &token,
        &format!("/views/{}", fixture.view.expect("vue")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(row_count(&html), 10);
    let first = html.find("facture-09.txt").expect("f09");
    let rapport = html.find("Rapport-annuel.txt").expect("rapport");
    assert!(first < rapport);
    assert!(!html.contains("facture-24.txt"));
    assert!(
        html.contains(&format!(
            "name=\"view\" value=\"{}\"",
            fixture.view.expect("vue")
        )),
        "{html}"
    );
}

#[tokio::test]
async fn missing_view_is_not_found() {
    let fixture = fixture(None).await;
    let token = login(&fixture.router).await;
    let (status, _) = page(&fixture.router, &token, "/views/9999").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let _ = &fixture.app;
}
