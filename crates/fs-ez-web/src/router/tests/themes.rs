use super::*;

#[tokio::test]
async fn theme_query_overrides_rendered_attribute() {
    let temp = TempDir::new().expect("tempdir");
    let router = build_router(state(&temp));
    let response = login(&router, None).await;
    let token = session_token(&response);

    let response = send(&router, with_cookie(get("/?theme=dark"), &token)).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(body_text(response).await.contains("data-theme=\"dark\""));

    let response = send(&router, with_cookie(get("/?theme=bogus"), &token)).await;
    assert_eq!(response.status(), StatusCode::OK);
    // La valeur invalide est ignorée : le thème valide précédent reste en base.
    assert!(body_text(response).await.contains("data-theme=\"dark\""));
}
