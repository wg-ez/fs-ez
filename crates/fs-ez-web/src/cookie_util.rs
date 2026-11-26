//! Squelette de cookie partagé : path, SameSite, secure, HttpOnly, Max-Age.

use axum::http::{HeaderMap, header};
use axum_extra::extract::cookie::{Cookie, SameSite};
use time::Duration;

pub(crate) fn cookie(
    name: &'static str,
    value: &str,
    http_only: bool,
    max_age: Duration,
    secure: bool,
) -> Cookie<'static> {
    Cookie::build((name, value.to_string()))
        .path("/")
        .http_only(http_only)
        .same_site(SameSite::Lax)
        .secure(secure)
        .max_age(max_age)
        .build()
}

/// `Secure` sauf `http://` explicite d'Origin/Referer, absent ou inconnu = `Secure`.
pub(crate) fn secure_from_headers(headers: &HeaderMap) -> bool {
    for name in [header::ORIGIN, header::REFERER] {
        let Some(value) = headers.get(name).and_then(|value| value.to_str().ok()) else {
            continue;
        };
        let value = value.trim_start();
        if value
            .get(..8)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("https://"))
        {
            return true;
        }
        if value
            .get(..7)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("http://"))
        {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests;
