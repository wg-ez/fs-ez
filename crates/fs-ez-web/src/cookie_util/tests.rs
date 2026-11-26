use super::*;
use axum::http::HeaderValue;

#[test]
fn applique_les_attributs_communs() {
    let built = cookie("fs-ez_test", "valeur", true, Duration::seconds(42), true);
    assert_eq!(built.name(), "fs-ez_test");
    assert_eq!(built.value(), "valeur");
    assert_eq!(built.path(), Some("/"));
    assert_eq!(built.same_site(), Some(SameSite::Lax));
    assert_eq!(built.secure(), Some(true));
    assert_eq!(built.http_only(), Some(true));
    assert_eq!(built.max_age(), Some(Duration::seconds(42)));
    assert_eq!(built.domain(), None);
}

#[test]
fn sans_http_only_ni_secure() {
    let built = cookie("fs-ez_test", "valeur", false, Duration::ZERO, false);
    assert_eq!(built.http_only(), Some(false));
    assert_eq!(built.secure(), Some(false));
    assert_eq!(built.max_age(), Some(Duration::ZERO));
    assert_eq!(built.value(), "valeur");
}

#[test]
fn secure_suit_le_schema_origin_ou_referer() {
    let mut headers = HeaderMap::new();
    assert!(secure_from_headers(&headers));

    headers.insert(header::ORIGIN, HeaderValue::from_static("http://fs.local"));
    assert!(!secure_from_headers(&headers));

    headers.insert(header::ORIGIN, HeaderValue::from_static("https://fs.local"));
    assert!(secure_from_headers(&headers));

    let mut referer = HeaderMap::new();
    referer.insert(
        header::REFERER,
        HeaderValue::from_static("http://fs.local/login"),
    );
    assert!(!secure_from_headers(&referer));

    let mut unknown = HeaderMap::new();
    unknown.insert(header::ORIGIN, HeaderValue::from_static("null"));
    assert!(secure_from_headers(&unknown));
}
