use axum::http::{HeaderMap, HeaderValue, header::ACCEPT};

use super::accepts_json;

fn headers(value: &'static str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static(value));
    headers
}

#[test]
fn header_absent_refuse_json() {
    assert!(!accepts_json(&HeaderMap::new()));
}

#[test]
fn json_seul_accepte() {
    assert!(accepts_json(&headers("application/json")));
}

#[test]
fn json_avec_charset_accepte() {
    assert!(accepts_json(&headers("application/json; charset=utf-8")));
}

#[test]
fn liste_avec_q_accepte() {
    assert!(accepts_json(&headers("text/html, application/json;q=0.9")));
}

#[test]
fn autre_type_refuse_json() {
    assert!(!accepts_json(&headers("text/html")));
}
