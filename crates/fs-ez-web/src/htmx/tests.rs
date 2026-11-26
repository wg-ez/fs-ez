use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue};
use axum::response::Response;

use super::{is_htmx, with_trigger};

#[test]
fn detecte_la_presence_de_hx_request() {
    let mut headers = HeaderMap::new();
    assert!(!is_htmx(&headers));
    headers.insert("hx-request", HeaderValue::from_static("true"));
    assert!(is_htmx(&headers));
}

#[test]
fn insere_le_trigger() {
    let response = with_trigger(Response::new(Body::empty()), "tags-changed");
    assert_eq!(
        response.headers().get("hx-trigger"),
        Some(&HeaderValue::from_static("tags-changed"))
    );
}
