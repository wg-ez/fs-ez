use super::support::*;
use crate::common::*;

#[tokio::test]
async fn validators_are_http_dates_and_conditionals_are_honored() {
    let dav = setup_named("probe.pdf", b"0123456789abcdef").await;
    let router = dav.fixture.router();
    let uri = tag_file_uri(TAG, &dav.alias);

    let response = send(
        &router,
        dav_request(Method::GET, &uri, &dav.token, Body::empty()),
    )
    .await;
    let last_modified = header_string(&response, header::LAST_MODIFIED);
    assert!(last_modified.ends_with(" GMT"), "{last_modified}");
    assert!(!last_modified.contains('+'), "{last_modified}");
    let etag = header_string(&response, header::ETAG);
    assert!(etag.starts_with('"') && etag.ends_with('"'), "{etag}");

    let response = send(
        &router,
        with_header(
            dav_request(Method::GET, &uri, &dav.token, Body::empty()),
            "if-none-match",
            &etag,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(header_string(&response, header::ETAG), etag);
    assert!(body_bytes(response).await.is_empty());

    let response = send(
        &router,
        with_header(
            dav_request(Method::GET, &uri, &dav.token, Body::empty()),
            "if-match",
            "\"autre\"",
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::PRECONDITION_FAILED);

    let response = send(
        &router,
        with_header(
            dav_request(Method::GET, &uri, &dav.token, Body::empty()),
            "if-modified-since",
            &last_modified,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
}

#[tokio::test]
async fn if_range_mismatch_serves_the_full_representation() {
    let dav = setup_named("probe.pdf", b"0123456789abcdef").await;
    let router = dav.fixture.router();
    let uri = tag_file_uri(TAG, &dav.alias);

    let response = send(
        &router,
        dav_request(Method::GET, &uri, &dav.token, Body::empty()),
    )
    .await;
    let etag = header_string(&response, header::ETAG);

    let request = with_header(
        range_request(Method::GET, &uri, &dav.token, "bytes=0-3"),
        "if-range",
        &etag,
    );
    let response = send(&router, request).await;
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(body_bytes(response).await, b"0123");

    let request = with_header(
        range_request(Method::GET, &uri, &dav.token, "bytes=0-3"),
        "if-range",
        "\"stale\"",
    );
    let response = send(&router, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_bytes(response).await, b"0123456789abcdef");
}

#[tokio::test]
async fn propfind_honors_requested_properties_and_propname() {
    let dav = setup_named("probe.pdf", b"hello").await;
    let router = dav.fixture.router();
    let uri = tag_file_uri(TAG, &dav.alias);

    let body = "<?xml version=\"1.0\"?><D:propfind xmlns:D=\"DAV:\"><D:prop>\
         <D:getcontentlength/><D:getcontenttype/><D:inexistante/></D:prop></D:propfind>";
    let response = send(&router, propfind_with_body(&uri, &dav.token, "0", body)).await;
    assert_eq!(response.status(), StatusCode::MULTI_STATUS);
    let body = body_text(response).await;
    assert!(
        body.contains("<D:getcontentlength>5</D:getcontentlength>"),
        "{body}"
    );
    assert!(
        body.contains("<D:getcontenttype>application/pdf</D:getcontenttype>"),
        "{body}"
    );
    assert!(!body.contains("<D:getetag>"), "{body}");

    let names = "<?xml version=\"1.0\"?><D:propfind xmlns:D=\"DAV:\"><D:propname/></D:propfind>";
    let response = send(&router, propfind_with_body(&uri, &dav.token, "0", names)).await;
    assert_eq!(response.status(), StatusCode::MULTI_STATUS);
    let body = body_text(response).await;
    assert!(body.contains("<D:getcontentlength>"), "{body}");
    assert!(
        !body.contains("<D:getcontentlength>5</D:getcontentlength>"),
        "{body}"
    );
    assert!(body.contains("<D:supportedlock>"), "{body}");
}

#[tokio::test]
async fn propfind_sanitizes_control_characters_in_displayname() {
    let dav = setup_named("note\nname.txt", b"x").await;
    let router = dav.fixture.router();

    let response = send(
        &router,
        propfind(&format!("/dav/tags/{TAG}/{FILES}/"), &dav.token, "1"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::MULTI_STATUS);
    let body = body_text(response).await;
    assert!(
        body.contains("<D:href>/dav/tags/work/All%20Files/note_name.txt</D:href>"),
        "{body}"
    );
    assert!(!body.contains("note\nname.txt"), "{body}");
}
