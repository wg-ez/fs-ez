use axum::http::HeaderMap;
use axum::http::HeaderValue;
use axum::http::header::ACCEPT_LANGUAGE;

use super::{preferred_lang, safe_next};

#[test]
fn safe_next_rejects_foreign_targets() {
    assert_eq!(safe_next(None), "/");
    assert_eq!(safe_next(Some("/inbox")), "/inbox");
    assert_eq!(safe_next(Some("//evil.example")), "/");
    assert_eq!(safe_next(Some("https://evil.example")), "/");
    assert_eq!(safe_next(Some("/\\evil")), "/");
    assert_eq!(safe_next(Some("/\r\nSet-Cookie: x")), "/");
    assert_eq!(safe_next(Some("/\t/evil.example")), "/");
    assert_eq!(safe_next(Some("/\n/evil.example")), "/");
    assert_eq!(safe_next(Some("/\0/evil.example")), "/");
    assert_eq!(safe_next(Some("/files?tag=a")), "/files?tag=a");
    assert_eq!(safe_next(Some("/views/3")), "/views/3");
}

#[test]
fn preferred_lang_delegates_to_regional_parsing() {
    let mut headers = HeaderMap::new();
    assert_eq!(preferred_lang(&headers), "en");

    headers.insert(
        ACCEPT_LANGUAGE,
        HeaderValue::from_static("fr-CA,fr;q=0.9,en;q=0.8"),
    );
    assert_eq!(preferred_lang(&headers), "fr");

    headers.insert(
        ACCEPT_LANGUAGE,
        HeaderValue::from_static("en-US,en;q=0.9,fr;q=0.8"),
    );
    assert_eq!(preferred_lang(&headers), "en");
}
