use axum::http::{HeaderMap, HeaderValue};
use axum_extra::extract::cookie::SameSite;

use super::*;

#[test]
fn cookie_attributes_match_contract() {
    let cookie = build_cookie("jeton", true);
    assert_eq!(cookie.name(), COOKIE_NAME);
    assert_eq!(cookie.value(), "jeton");
    assert_eq!(cookie.path(), Some("/"));
    assert_eq!(cookie.http_only(), Some(true));
    assert_eq!(cookie.same_site(), Some(SameSite::Lax));
    assert_eq!(cookie.secure(), Some(true));
    assert_eq!(cookie.max_age(), Some(Duration::seconds(MAX_AGE_SECONDS)));
    assert_eq!(cookie.domain(), None);

    let plain_http = build_cookie("jeton", false);
    assert_eq!(plain_http.secure(), Some(false));

    let cleared = clear_cookie(true);
    assert_eq!(cleared.max_age(), Some(Duration::ZERO));
}

#[test]
fn reads_the_session_cookie_only() {
    let mut headers = HeaderMap::new();
    headers.append(
        "cookie",
        HeaderValue::from_static("autre=1; fs-ez_session=abc"),
    );
    assert_eq!(read_cookie(&headers).as_deref(), Some("abc"));

    let empty = HeaderMap::new();
    assert!(read_cookie(&empty).is_none());
}
