use super::{local_target, request_target};
use axum::http::header::REFERER;
use axum::http::{HeaderMap, HeaderValue};

#[test]
fn local_target_accepts_legitimate_paths() {
    assert_eq!(local_target("/").as_deref(), Some("/"));
    assert_eq!(
        local_target("/files?tag=a").as_deref(),
        Some("/files?tag=a")
    );
    assert_eq!(local_target("/views/3").as_deref(), Some("/views/3"));
    assert_eq!(
        local_target("/views?x=1#top").as_deref(),
        Some("/views?x=1#top")
    );
}

#[test]
fn local_target_rejects_foreign_targets() {
    assert_eq!(local_target("//evil.example"), None);
    assert_eq!(local_target("https://evil.example"), None);
    assert_eq!(local_target("/\\evil"), None);
    assert_eq!(local_target("evil.example"), None);
}

#[test]
fn local_target_requires_ascii() {
    assert_eq!(local_target("/café"), None);
    assert_eq!(local_target("/%C3%A9").as_deref(), Some("/%C3%A9"));
    assert_eq!(local_target("/ok?x=1").as_deref(), Some("/ok?x=1"));
}

#[test]
fn local_target_rejects_control_characters() {
    assert_eq!(local_target("/\t/evil.example"), None);
    assert_eq!(local_target("/\n/evil.example"), None);
    assert_eq!(local_target("/\0/evil.example"), None);
    assert_eq!(local_target("/\r\nSet-Cookie: x"), None);
    assert_eq!(local_target("/files\t?tag=a"), None);
}

#[test]
fn request_target_keeps_local_path_and_query() {
    assert_eq!(
        request_target("/files?tag=a"),
        Some("/files?tag=a".to_string())
    );
    assert_eq!(
        request_target("https://evil.example/files"),
        Some("/files".to_string())
    );
    assert_eq!(request_target("//evil.example/files"), None);
    assert_eq!(request_target("/files?a=\t/evil"), None);
}

#[test]
fn referer_target_falls_back_on_foreign_or_control_referer() {
    let mut headers = HeaderMap::new();
    assert_eq!(super::referer_target(&headers, "/"), "/");

    headers.insert(
        REFERER,
        HeaderValue::from_static("https://app.example/files?x=1"),
    );
    assert_eq!(super::referer_target(&headers, "/"), "/files?x=1");

    headers.insert(REFERER, HeaderValue::from_static("//evil.example/files"));
    assert_eq!(super::referer_target(&headers, "/"), "/");
}
