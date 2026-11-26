use super::*;

#[tokio::test]
async fn auth_redirects_json_refusal_login_and_logout() {
    let fixture = web_fixture().await;
    let router = fixture.router();

    let response = send(&router, get("/")).await;
    assert_status(&response, StatusCode::SEE_OTHER);
    assert_eq!(location(&response), "/login?next=%2F");

    let response = send(&router, get("/api/files")).await;
    assert_status(&response, StatusCode::UNAUTHORIZED);

    let response = send(
        &router,
        post_form("/login", &login_body("mauvais-mot-de-passe")),
    )
    .await;
    assert_status(&response, StatusCode::OK);
    let html = body_text(response).await;
    assert!(html.contains("Invalid credentials"));
    assert!(!html.to_ascii_lowercase().contains("unknown"));

    assert!(
        fixture
            .app
            .validate_session(&fixture.cookie)
            .expect("session")
            .is_some()
    );

    let response = fixture.post("/logout", "").await;
    assert_status(&response, StatusCode::SEE_OTHER);
    assert_eq!(location(&response), "/login");
    assert!(
        fixture
            .app
            .validate_session(&fixture.cookie)
            .expect("session")
            .is_none()
    );
}

#[tokio::test]
async fn cookie_attributes_are_secure_by_default() {
    let fixture = web_fixture().await;
    let cookie = &fixture.set_cookie;
    assert!(cookie.contains("HttpOnly"), "{cookie}");
    assert!(cookie.contains("SameSite=Lax"), "{cookie}");
    assert!(cookie.contains("Secure"), "{cookie}");
    assert!(cookie.contains("Max-Age="), "{cookie}");
    assert!(!cookie.contains("Domain"), "{cookie}");
}

#[tokio::test]
async fn without_javascript_pages_render_and_posts_redirect() {
    let fixture = web_fixture().await;
    let id = fixture.add_file("sansjs.txt", b"x", &[]);

    let response = fixture
        .send(with_cookie(
            with_referer(
                post_form(
                    "/partials/files/tags",
                    &format!("ids={id}&tags=sansjs&action=add"),
                ),
                "/?sort=name:asc",
            ),
            &fixture.cookie,
        ))
        .await;
    assert_status(&response, StatusCode::SEE_OTHER);
    assert_eq!(location(&response), "/?sort=name:asc");
    assert!(fixture.tag_names(id).contains(&"sansjs".to_string()));

    let response = fixture.post("/logout", "").await;
    assert_status(&response, StatusCode::SEE_OTHER);
}
