use axum::http::{HeaderValue, header};

use super::*;

fn headers(value: &'static str) -> HeaderMap {
    let mut map = HeaderMap::new();
    map.insert(header::AUTHORIZATION, HeaderValue::from_static(value));
    map
}

#[test]
fn absent_header_is_not_an_error() {
    assert_eq!(parse_bearer(&HeaderMap::new()).expect("absent"), None);
}

#[test]
fn bearer_scheme_is_case_insensitive() {
    assert_eq!(
        parse_bearer(&headers("bearer jeton")).expect("valide"),
        Some("jeton".to_string())
    );
    assert_eq!(
        parse_bearer(&headers("Bearer   jeton  ")).expect("valide"),
        Some("jeton".to_string())
    );
}

#[test]
fn malformed_headers_are_unauthorized() {
    for raw in ["Bearer", "Bearer ", "Basic jeton", "jeton"] {
        assert!(matches!(
            parse_bearer(&headers(raw)),
            Err(WebError::Unauthorized)
        ));
    }
}
