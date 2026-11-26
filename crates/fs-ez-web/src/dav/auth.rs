//! Authentification HTTP Basic : le mot de passe est un jeton d'API fs-ez.

use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use crate::backend::{Backend, blocking};
use crate::state::AppState;

use super::logging;

const REALM: &str = "Basic realm=\"fs-ez\"";

pub async fn basic_auth<B: Backend>(
    State(state): State<AppState<B>>,
    req: Request,
    next: Next,
) -> Response {
    let Some(token) = parse_basic(req.headers()) else {
        logging::auth_failed("missing_or_invalid_basic");
        return unauthorized();
    };
    let record = match state.dav.tokens().get(&token) {
        Some(Some(record)) => {
            // Reset CLI externe : la generation doit etre relue avant de servir une entree cachee.
            let current = match blocking(state.backend.app(), |app| app.credential_generation())
                .await
            {
                Ok(current) => current,
                Err(error) => {
                    tracing::warn!(target: logging::TARGET, %error, "dav auth generation error");
                    logging::auth_failed("validation_error");
                    return unauthorized();
                }
            };
            if record.credential_generation != current {
                state.dav.tokens().remove(&token);
                logging::auth_failed("stale_credential_generation");
                return unauthorized();
            }
            Some(record)
        }
        Some(None) => None,
        None => {
            let lookup = token.clone();
            match blocking(state.backend.app(), move |app| {
                app.validate_api_token(&lookup)
            })
            .await
            {
                Ok(record) => {
                    state.dav.tokens().put(&token, record.clone());
                    record
                }
                Err(error) => {
                    tracing::warn!(target: logging::TARGET, %error, "dav auth validation error");
                    logging::auth_failed("validation_error");
                    return unauthorized();
                }
            }
        }
    };
    match record {
        Some(record) => {
            logging::auth_ok(record.id.0, &record.name);
            next.run(req).await
        }
        None => {
            logging::auth_failed("unknown_or_expired_token");
            unauthorized()
        }
    }
}

fn parse_basic(headers: &axum::http::HeaderMap) -> Option<String> {
    let raw = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, encoded) = raw.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("Basic") {
        return None;
    }
    let decoded = STANDARD.decode(encoded.trim()).ok()?;
    let credentials = String::from_utf8(decoded).ok()?;
    let (_, password) = credentials.split_once(':')?;
    if password.is_empty() {
        return None;
    }
    Some(password.to_string())
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [(header::WWW_AUTHENTICATE, HeaderValue::from_static(REALM))],
    )
        .into_response()
}
