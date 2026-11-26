//! Extraction des identifiants : cookie de session ou `Authorization: Bearer`.

use axum::http::{HeaderMap, header::AUTHORIZATION};

use crate::error::WebError;

/// Origine de l'authentification, utile pour restreindre certains endpoints à une session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthSource {
    Session,
    Bearer,
}

/// En-tête absent = `Ok(None)`, en-tête illisible ou schéma non Bearer = 401.
pub fn parse_bearer(headers: &HeaderMap) -> Result<Option<String>, WebError> {
    let Some(value) = headers.get(AUTHORIZATION) else {
        return Ok(None);
    };
    let value = value.to_str().map_err(|_| WebError::Unauthorized)?;
    let (scheme, token) = value.split_once(' ').ok_or(WebError::Unauthorized)?;
    if !scheme.eq_ignore_ascii_case("Bearer") {
        return Err(WebError::Unauthorized);
    }
    let token = token.trim();
    if token.is_empty() {
        return Err(WebError::Unauthorized);
    }
    Ok(Some(token.to_string()))
}

#[cfg(test)]
mod tests;
