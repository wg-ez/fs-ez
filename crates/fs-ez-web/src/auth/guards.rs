//! Guard d'authentification : cookie de session ou jeton Bearer, refus JSON pour `/api`.

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use fs_ez_app::AppError;
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};

use crate::api::auth::{AuthSource, parse_bearer};
use crate::auth::session::read_cookie;
use crate::backend::{Backend, blocking};
use crate::error::WebError;
use crate::negotiation;
use crate::state::AppState;

#[derive(Debug, Clone)]
pub struct SessionUser {
    pub username: String,
    pub credential_generation: i64,
}

pub async fn require_auth<B: Backend>(
    State(state): State<AppState<B>>,
    mut req: Request,
    next: Next,
) -> Response {
    if is_public(req.uri().path()) {
        return next.run(req).await;
    }
    if let Some(token) = read_cookie(req.headers()) {
        // Le token n'apparaît ni dans les logs ni dans un message d'erreur.
        match blocking(state.backend.app(), move |app| app.validate_session(&token)).await {
            Ok(Some(principal)) => {
                insert_user(
                    &mut req,
                    principal.username,
                    principal.credential_generation,
                    AuthSource::Session,
                );
                return next.run(req).await;
            }
            Ok(None) => {}
            Err(error) => return error.into_response(),
        }
    }
    if !wants_json(&req)
        && let Some(response) = no_user_redirect(&state).await
    {
        return response;
    }
    let bearer = match parse_bearer(req.headers()) {
        Ok(token) => token,
        Err(error) => return error.into_response(),
    };
    if let Some(token) = bearer {
        let found = blocking(state.backend.app(), move |app| {
            let Some(record) = app.validate_api_token(&token)? else {
                return Ok(None);
            };
            let user = app
                .store
                .get_user()?
                .ok_or_else(|| AppError::NotFound("user".to_string()))?;
            Ok(Some((record, user.username)))
        })
        .await;
        match found {
            Ok(Some((record, username))) => {
                insert_user(
                    &mut req,
                    username,
                    record.credential_generation,
                    AuthSource::Bearer,
                );
                return next.run(req).await;
            }
            Ok(None) => {}
            Err(error) => return error.into_response(),
        }
    }
    refuse(&req)
}

/// `Some(/setup)` tant qu'aucun compte n'existe : renvoi identique pour `/login` et le guard.
pub(crate) async fn no_user_redirect<B: Backend>(state: &AppState<B>) -> Option<Response> {
    match blocking(state.backend.app(), |app| app.has_user()).await {
        Ok(true) => None,
        Ok(false) => Some(Redirect::to("/setup").into_response()),
        Err(error) => Some(error.into_response()),
    }
}

/// `Some(/login)` dès qu'un compte existe : garde de l'onboarding idempotent.
pub(crate) async fn existing_user_redirect<B: Backend>(
    state: &AppState<B>,
) -> Result<Option<Response>, WebError> {
    let exists = blocking(state.backend.app(), |app| app.has_user()).await?;
    Ok(exists.then(|| Redirect::to("/login").into_response()))
}

fn insert_user(
    req: &mut Request,
    username: String,
    credential_generation: i64,
    source: AuthSource,
) {
    req.extensions_mut().insert(SessionUser {
        username,
        credential_generation,
    });
    req.extensions_mut().insert(source);
}

fn is_public(path: &str) -> bool {
    path == "/health"
        || path == "/login"
        || path == "/setup"
        || path == "/favicon.ico"
        || path.starts_with("/static/")
}

fn refuse(req: &Request) -> Response {
    if wants_json(req) {
        return WebError::AuthenticationRequired.into_response();
    }
    let target = req
        .uri()
        .path_and_query()
        .map_or("/", |value| value.as_str());
    let encoded = utf8_percent_encode(target, NON_ALPHANUMERIC);
    Redirect::to(&format!("/login?next={encoded}")).into_response()
}

fn wants_json(req: &Request) -> bool {
    req.uri().path().starts_with("/api") || negotiation::accepts_json(req.headers())
}
