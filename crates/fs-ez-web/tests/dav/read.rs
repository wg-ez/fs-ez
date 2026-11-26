use super::support::*;
use crate::common::*;
use fs_ez_core::virtual_file_name;

#[tokio::test]
async fn smart_views_list_and_serve_their_files() {
    let dav = setup().await;
    let router = dav.fixture.router();
    let loose_path = dav.fixture.root().join("loose.txt");
    std::fs::write(&loose_path, b"loose").expect("écriture");
    let loose = dav
        .fixture
        .app
        .ensure_file(&loose_path, fs_ez_app::EnsureOptions { force: false })
        .expect("fichier");
    let loose_alias = virtual_file_name(&loose);

    let body = propfind_body(&router, "/dav/untagged/", &dav.token, "1").await;
    assert!(body.contains(&loose_alias), "{body}");
    assert!(!body.contains(&dav.alias), "{body}");

    let response = dav_get(&router, &format!("/dav/untagged/{loose_alias}"), &dav.token).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_bytes(response).await, b"loose");

    let body = propfind_body(&router, "/dav/recently%20tagged/", &dav.token, "1").await;
    assert!(body.contains(&dav.alias), "{body}");
    assert!(!body.contains(&loose_alias), "{body}");

    for uri in ["/dav/recent%20files/", "/dav/recent-files/"] {
        let body = propfind_body(&router, uri, &dav.token, "1").await;
        assert!(body.contains(&dav.alias), "{body}");
        assert!(body.contains(&loose_alias), "{body}");
    }

    let body = propfind_body(&router, "/dav/recent/", &dav.token, "1").await;
    assert!(body.contains(&dav.alias), "{body}");
    assert!(!body.contains(&loose_alias), "{body}");
}

#[tokio::test]
async fn get_streams_file_content_and_reports_length() {
    let dav = setup().await;
    let router = dav.fixture.router();
    let uri = tag_file_uri(TAG, &dav.alias);

    let response = dav_get(&router, &uri, &dav.token).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header_string(&response, header::CONTENT_LENGTH), "5");
    assert_eq!(body_text(response).await, "hello");

    let response = send(
        &router,
        dav_request(Method::HEAD, &uri, &dav.token, Body::empty()),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header_string(&response, header::CONTENT_LENGTH), "5");
    assert!(body_text(response).await.is_empty());
}

#[tokio::test]
async fn get_and_head_report_real_file_size_after_external_change() {
    let dav = setup().await;
    let router = dav.fixture.router();
    let uri = tag_file_uri(TAG, &dav.alias);
    std::fs::write(dav.fixture.root().join("note.txt"), b"hello world").expect("écriture externe");

    let response = dav_get(&router, &uri, &dav.token).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header_string(&response, header::CONTENT_LENGTH), "11");
    assert_eq!(body_text(response).await, "hello world");

    let response = send(
        &router,
        dav_request(Method::HEAD, &uri, &dav.token, Body::empty()),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header_string(&response, header::CONTENT_LENGTH), "11");
    assert!(body_text(response).await.is_empty());
}

#[tokio::test]
async fn pdf_and_docx_keep_mime_and_exact_bytes() {
    let pdf_bytes = b"%PDF-1.7\n\x00\x01\x02binary\n%%EOF";
    let pdf = setup_named("probe.pdf", pdf_bytes).await;
    let router = pdf.fixture.router();
    let uri = tag_file_uri(TAG, &pdf.alias);

    let response = dav_get(&router, &uri, &pdf.token).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        header_string(&response, header::CONTENT_TYPE),
        "application/pdf"
    );
    assert_eq!(
        header_string(&response, header::CONTENT_LENGTH),
        pdf_bytes.len().to_string()
    );
    assert_eq!(body_bytes(response).await, pdf_bytes);

    let response = send(
        &router,
        dav_request(Method::HEAD, &uri, &pdf.token, Body::empty()),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        header_string(&response, header::CONTENT_TYPE),
        "application/pdf"
    );
    assert_eq!(
        header_string(&response, header::CONTENT_LENGTH),
        pdf_bytes.len().to_string()
    );
    assert!(body_bytes(response).await.is_empty());

    let body = propfind_body(
        &router,
        &format!("/dav/tags/{TAG}/{FILES}/"),
        &pdf.token,
        "1",
    )
    .await;
    assert!(
        body.contains("<D:getcontenttype>application/pdf</D:getcontenttype>"),
        "{body}"
    );

    let docx_bytes = b"PK\x03\x04\x00\x00fake-docx-payload";
    let docx = setup_named("Document1.docx", docx_bytes).await;
    let router = docx.fixture.router();
    let uri = tag_file_uri(TAG, &docx.alias);
    let response = dav_get(&router, &uri, &docx.token).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        header_string(&response, header::CONTENT_TYPE),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
    );
    assert_eq!(
        header_string(&response, header::CONTENT_LENGTH),
        docx_bytes.len().to_string()
    );
    assert_eq!(body_bytes(response).await, docx_bytes);
}

#[tokio::test]
async fn range_requests_serve_only_the_requested_segment() {
    let dav = setup_named("probe.pdf", b"0123456789abcdef").await;
    let router = dav.fixture.router();
    let uri = tag_file_uri(TAG, &dav.alias);

    let response = dav_get(&router, &uri, &dav.token).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header_string(&response, header::ACCEPT_RANGES), "bytes");

    let response = send(
        &router,
        range_request(Method::GET, &uri, &dav.token, "bytes=0-3"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        header_string(&response, header::CONTENT_RANGE),
        "bytes 0-3/16"
    );
    assert_eq!(header_string(&response, header::CONTENT_LENGTH), "4");
    assert_eq!(header_string(&response, header::ACCEPT_RANGES), "bytes");
    assert_eq!(body_bytes(response).await, b"0123");

    let response = send(
        &router,
        range_request(Method::GET, &uri, &dav.token, "bytes=4-"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        header_string(&response, header::CONTENT_RANGE),
        "bytes 4-15/16"
    );
    assert_eq!(header_string(&response, header::CONTENT_LENGTH), "12");
    assert_eq!(body_bytes(response).await, b"456789abcdef");

    let response = send(
        &router,
        range_request(Method::GET, &uri, &dav.token, "bytes=-5"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        header_string(&response, header::CONTENT_RANGE),
        "bytes 11-15/16"
    );
    assert_eq!(header_string(&response, header::CONTENT_LENGTH), "5");
    assert_eq!(body_bytes(response).await, b"bcdef");

    let response = send(
        &router,
        range_request(Method::GET, &uri, &dav.token, "bytes=0-99"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        header_string(&response, header::CONTENT_RANGE),
        "bytes 0-15/16"
    );

    let response = send(
        &router,
        range_request(Method::GET, &uri, &dav.token, "bytes=20-"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(
        header_string(&response, header::CONTENT_RANGE),
        "bytes */16"
    );
    assert!(body_bytes(response).await.is_empty());

    let response = send(
        &router,
        range_request(Method::GET, &uri, &dav.token, "bytes=0-1,4-5"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    let content_type = header_string(&response, header::CONTENT_TYPE);
    assert!(content_type.starts_with("multipart/byteranges; boundary="));
    let boundary = content_type
        .split("boundary=")
        .nth(1)
        .expect("boundary")
        .to_string();
    let body = body_text(response).await;
    assert!(body.contains(&format!("--{boundary}")));
    assert!(body.contains("Content-Range: bytes 0-1/16"));
    assert!(body.contains("Content-Range: bytes 4-5/16"));
    assert!(body.contains("01"));
    assert!(body.contains("45"));
    assert!(body.contains(&format!("--{boundary}--")));

    // HEAD ignore Range : corps et plage complets, jamais 206 sans lecture.
    let response = send(
        &router,
        range_request(Method::HEAD, &uri, &dav.token, "bytes=0-3"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(header_string(&response, header::CONTENT_LENGTH), "16");
    assert!(header_string(&response, header::CONTENT_RANGE).is_empty());
    assert!(body_bytes(response).await.is_empty());
}
