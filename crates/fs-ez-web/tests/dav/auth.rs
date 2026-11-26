use super::support::*;
use crate::common::*;

#[tokio::test]
async fn basic_auth_requires_a_valid_api_token() {
    let dav = setup().await;
    let router = dav.fixture.router();

    let response = send(&router, get("/dav/")).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        header_string(&response, header::WWW_AUTHENTICATE),
        "Basic realm=\"fs-ez\""
    );

    let response = send(
        &router,
        dav_request(Method::OPTIONS, "/dav/", "bad", Body::empty()),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let response = send(
        &router,
        dav_request(Method::OPTIONS, "/dav/", &dav.token, Body::empty()),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        header_string(&response, header::HeaderName::from_static("dav")),
        "1, 2"
    );
    assert!(header_string(&response, header::ALLOW).contains("PROPFIND"));
    assert!(header_string(&response, header::ALLOW).contains("LOCK"));
    // DAV ne porte pas les en-têtes anti-iframe de l'UI web.
    assert!(
        response
            .headers()
            .get(header::CONTENT_SECURITY_POLICY)
            .is_none()
    );
    assert!(response.headers().get(header::X_FRAME_OPTIONS).is_none());
    assert_eq!(
        header_string(&response, header::X_CONTENT_TYPE_OPTIONS),
        "nosniff"
    );
}

#[tokio::test]
async fn propfind_root_tag_and_view_lists_aliases() {
    let dav = setup().await;
    let router = dav.fixture.router();

    let response = send(&router, propfind("/dav/", &dav.token, "1")).await;
    assert_eq!(response.status(), StatusCode::MULTI_STATUS);
    let body = body_text(response).await;
    assert!(body.contains("<D:resourcetype><D:collection>"));
    assert!(body.contains("<D:href>/dav/</D:href>"), "{body}");
    assert!(body.contains("<D:href>/dav/tags/</D:href>"), "{body}");
    assert!(body.contains("<D:href>/dav/views/</D:href>"), "{body}");
    assert!(body.contains("<D:href>/dav/untagged/</D:href>"), "{body}");
    assert!(
        body.contains("<D:href>/dav/recently%20tagged/</D:href>"),
        "{body}"
    );
    assert!(
        body.contains("<D:href>/dav/recent%20files/</D:href>"),
        "{body}"
    );
    assert!(!body.contains("/dav/files/"), "{body}");
    assert!(body.contains("tags"));
    assert!(body.contains("views"));

    let response = send(&router, propfind("/dav/tags/", &dav.token, "1")).await;
    assert_eq!(response.status(), StatusCode::MULTI_STATUS);
    let body = body_text(response).await;
    assert!(body.contains(TAG));
    assert!(body.contains("<D:href>/dav/tags/</D:href>"), "{body}");
    assert!(
        body.contains(&format!("<D:href>/dav/tags/{TAG}/</D:href>")),
        "{body}"
    );

    let response = send(
        &router,
        propfind(&format!("/dav/tags/{TAG}/"), &dav.token, "1"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::MULTI_STATUS);
    let body = body_text(response).await;
    assert!(
        body.contains(&format!("<D:href>/dav/tags/{TAG}/{}<", dav.alias)),
        "{body}"
    );
    assert!(
        !body.contains(&format!("<D:href>/dav/tags/{TAG}/{FILES}/</D:href>")),
        "{body}"
    );
    assert!(
        body.contains(&format!("<D:href>/dav/tags/{TAG}/</D:href>")),
        "{body}"
    );

    let response = send(
        &router,
        propfind(&format!("/dav/tags/{TAG}/{FILES}/"), &dav.token, "1"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::MULTI_STATUS);
    let body = body_text(response).await;
    assert!(body.contains(&dav.alias), "{body}");
    assert!(body.contains("note.txt"), "{body}");
    assert!(
        body.contains(&format!("<D:href>/dav/tags/{TAG}/{FILES}/{}<", dav.alias)),
        "{body}"
    );

    let response = send(&router, propfind("/dav/views/", &dav.token, "1")).await;
    assert_eq!(response.status(), StatusCode::MULTI_STATUS);
    let body = body_text(response).await;
    assert!(body.contains(VIEW_GROUP));
    assert!(body.contains("<D:href>/dav/views/</D:href>"), "{body}");
    assert!(
        body.contains(&format!("<D:href>/dav/views/{VIEW_GROUP}/</D:href>")),
        "{body}"
    );

    let response = send(
        &router,
        propfind(&format!("/dav/views/{VIEW_GROUP}/"), &dav.token, "1"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::MULTI_STATUS);
    let body = body_text(response).await;
    assert!(body.contains(VIEW));
    assert!(
        body.contains(&format!("<D:href>/dav/views/{VIEW_GROUP}/{VIEW}/</D:href>")),
        "{body}"
    );

    let response = send(
        &router,
        propfind(&format!("/dav/views/{VIEW_GROUP}/{VIEW}/"), &dav.token, "1"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::MULTI_STATUS);
    let body = body_text(response).await;
    assert!(body.contains(&dav.alias));
    assert!(
        body.contains(&format!("<D:href>/dav/views/{VIEW_GROUP}/{VIEW}/</D:href>")),
        "{body}"
    );
}

#[tokio::test]
async fn revoked_token_stays_valid_only_for_the_cache_window() {
    use std::time::Duration;

    let dav = setup().await;
    let router = dav.fixture.router();
    let response = send(
        &router,
        dav_request(Method::OPTIONS, "/dav/", &dav.token, Body::empty()),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let token = dav
        .fixture
        .app
        .list_api_tokens()
        .expect("jetons")
        .into_iter()
        .find(|record| record.name == "dav")
        .expect("jeton");
    dav.fixture
        .app
        .revoke_api_token(token.id)
        .expect("révocation");

    // Cache court : la révocation s'applique au plus tard après le TTL.
    let response = send(
        &router,
        dav_request(Method::OPTIONS, "/dav/", &dav.token, Body::empty()),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    tokio::time::sleep(Duration::from_millis(3200)).await;
    let response = send(
        &router,
        dav_request(Method::OPTIONS, "/dav/", &dav.token, Body::empty()),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn reset_revokes_cached_token_immediately() {
    use fs_ez_app::sharded_path;
    use std::ffi::OsStr;

    let (fixture, token) = inbox_fixture().await;
    let router = fixture.router();
    let response = send(&router, propfind("/dav/", &token, "0")).await;
    assert_eq!(response.status(), StatusCode::MULTI_STATUS);

    fixture
        .app
        .reset_user(USERNAME, "new-password-1234")
        .expect("reset");

    // Fenetre TTL en cours : la generation relue doit refuser avant expiration du cache.
    let response = send(
        &router,
        dav_request(
            Method::PUT,
            "/dav/inbox/revoked.txt",
            &token,
            Body::from("stale"),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(!fixture.inbox().join("revoked.txt").exists());
    assert!(!sharded_path(&fixture.files(), OsStr::new("revoked.txt")).exists());

    let response = send(&router, bearer_request(Method::GET, "/api/session", &token)).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let generation = fixture.app.credential_generation().expect("generation");
    let fresh = fixture
        .app
        .create_api_token_for_generation("fresh", None, generation)
        .expect("jeton")
        .token;
    let response = send(
        &router,
        dav_request(
            Method::PUT,
            "/dav/inbox/fresh.txt",
            &fresh,
            Body::from("fresh"),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(
        std::fs::read(sharded_path(&fixture.files(), OsStr::new("fresh.txt"))).expect("fichier"),
        b"fresh"
    );
}
