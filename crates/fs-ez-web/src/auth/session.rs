//! Cookie de session : token opaque unique, attributs figés.

use axum::http::HeaderMap;
use axum::http::header::SET_COOKIE;
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use time::Duration;

use crate::cookie_util;

pub const COOKIE_NAME: &str = "fs-ez_session";
/// 30 jours, aligné sur la durée de vie serveur des sessions.
const MAX_AGE_SECONDS: i64 = 2_592_000;

pub fn build_cookie(token: &str, secure: bool) -> Cookie<'static> {
    cookie_util::cookie(
        COOKIE_NAME,
        token,
        true,
        Duration::seconds(MAX_AGE_SECONDS),
        secure,
    )
}

/// Même nom et mêmes attributs que la création, `Max-Age=0` pour effacer.
pub fn clear_cookie(secure: bool) -> Cookie<'static> {
    cookie_util::cookie(COOKIE_NAME, "", true, Duration::ZERO, secure)
}

/// Pose le cookie de session puis redirige : émission identique pour login et setup.
pub fn login_response(headers: &HeaderMap, token: &str, location: &str) -> Response {
    let cookie = build_cookie(token, cookie_util::secure_from_headers(headers));
    ([(SET_COOKIE, cookie.to_string())], Redirect::to(location)).into_response()
}

pub fn read_cookie(headers: &HeaderMap) -> Option<String> {
    CookieJar::from_headers(headers)
        .get(COOKIE_NAME)
        .map(|cookie| cookie.value().to_string())
        .filter(|token| !token.is_empty())
}

#[cfg(test)]
mod tests;
