use std::time::Duration;

use super::support::*;
use crate::common::*;

#[tokio::test]
async fn lock_timeout_expires_and_allows_put() {
    let dav = setup().await;
    let router = dav.fixture.router();
    let uri = tag_file_uri(TAG, &dav.alias);
    let lock_body = "<?xml version=\"1.0\"?><D:lockinfo xmlns:D=\"DAV:\">\
        <D:lockscope><D:exclusive/></D:lockscope><D:locktype><D:write/></D:locktype>\
        <D:owner>tests</D:owner></D:lockinfo>";

    let response = send(
        &router,
        with_header(
            dav_request(
                Method::from_bytes(b"LOCK").expect("méthode"),
                &uri,
                &dav.token,
                Body::from(lock_body),
            ),
            "timeout",
            "Second-1",
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let token = header_string(&response, header::HeaderName::from_static("lock-token"));
    let body = body_text(response).await;
    assert!(body.contains("Second-1"), "{body}");

    tokio::time::sleep(Duration::from_millis(1200)).await;

    let response = send(
        &router,
        propfind_with_body(
            &uri,
            &dav.token,
            "0",
            "<?xml version=\"1.0\"?><D:propfind xmlns:D=\"DAV:\">\
                <D:prop><D:lockdiscovery/></D:prop></D:propfind>",
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::MULTI_STATUS);
    let body = body_text(response).await;
    assert!(!body.contains("locktoken"), "{body}");
    assert!(!body.contains(&token), "{body}");

    let response = send(
        &router,
        dav_request(Method::PUT, &uri, &dav.token, Body::from("after expiry")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn lock_unlock_roundtrip_and_conflicts() {
    let dav = setup().await;
    let router = dav.fixture.router();
    let uri = tag_file_uri(TAG, &dav.alias);
    let lock_body = "<?xml version=\"1.0\"?><D:lockinfo xmlns:D=\"DAV:\">\
        <D:lockscope><D:exclusive/></D:lockscope><D:locktype><D:write/></D:locktype>\
        <D:owner>tests</D:owner></D:lockinfo>";

    let response = send(
        &router,
        dav_request(
            Method::from_bytes(b"LOCK").expect("méthode"),
            &uri,
            &dav.token,
            Body::from(lock_body),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let token = header_string(&response, header::HeaderName::from_static("lock-token"));
    assert!(token.starts_with('<') && token.ends_with('>'), "{token}");
    let body = body_text(response).await;
    assert!(body.contains("<D:lockdiscovery>"), "{body}");
    assert!(body.contains("<D:locktoken>"), "{body}");

    let response = send(
        &router,
        dav_request(
            Method::from_bytes(b"LOCK").expect("méthode"),
            &uri,
            &dav.token,
            Body::from(lock_body),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::LOCKED);

    let response = send(
        &router,
        dav_request(Method::PUT, &uri, &dav.token, Body::from("blocked")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::LOCKED);

    let request = with_header(
        dav_request(Method::PUT, &uri, &dav.token, Body::from("allowed")),
        "if",
        &format!("({token})"),
    );
    let response = send(&router, request).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let response = send(
        &router,
        with_header(
            dav_request(
                Method::from_bytes(b"UNLOCK").expect("méthode"),
                &uri,
                &dav.token,
                Body::empty(),
            ),
            "lock-token",
            "<opaquelocktoken:fs-ez-inconnu>",
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);

    let response = send(
        &router,
        with_header(
            dav_request(
                Method::from_bytes(b"UNLOCK").expect("méthode"),
                &uri,
                &dav.token,
                Body::empty(),
            ),
            "lock-token",
            &token,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let response = send(
        &router,
        dav_request(Method::PUT, &uri, &dav.token, Body::from("free")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}
