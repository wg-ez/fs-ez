//! API JSON des jetons d'API : réservée à une session, jamais le hash ni un jeton existant.

use axum::Extension;
use axum::extract::{Path, State};
use axum::response::Response;
use fs_ez_core::ApiTokenId;
use fs_ez_core::datetime::parse_rfc3339;
use serde::Deserialize;

use crate::api::auth::AuthSource;
use crate::auth::SessionUser;
use crate::backend::{Backend, blocking};
use crate::error::WebError;
use crate::state::AppState;

use super::dto::{TokenCreatedDto, TokenDto};
use super::errors::{ApiJson, delete_ok, json_created, json_ok, parse_id};

/// Un jeton compromis ne doit pouvoir ni lister, ni créer, ni révoquer les autres.
fn require_session(source: AuthSource) -> Result<(), WebError> {
    match source {
        AuthSource::Session => Ok(()),
        AuthSource::Bearer => Err(WebError::Forbidden),
    }
}

#[derive(Debug, serde::Serialize)]
struct TokensDto {
    tokens: Vec<TokenDto>,
}

pub async fn list<B: Backend>(
    State(state): State<AppState<B>>,
    Extension(source): Extension<AuthSource>,
) -> Result<Response, WebError> {
    require_session(source)?;
    let tokens = blocking(state.backend.app(), |app| {
        Ok(app
            .list_api_tokens()?
            .into_iter()
            .map(TokenDto::from_record)
            .collect::<Vec<_>>())
    })
    .await?;
    Ok(json_ok(TokensDto { tokens }))
}

#[derive(Debug, Deserialize)]
pub struct CreateToken {
    pub name: String,
    #[serde(default)]
    pub expires_at: Option<String>,
}

pub async fn create<B: Backend>(
    State(state): State<AppState<B>>,
    Extension(source): Extension<AuthSource>,
    Extension(user): Extension<SessionUser>,
    ApiJson(body): ApiJson<CreateToken>,
) -> Result<Response, WebError> {
    require_session(source)?;
    let expires_at = match body.expires_at.as_deref() {
        Some(raw) => Some(
            parse_rfc3339(raw.trim())
                .map_err(|_| WebError::BadRequest("invalid expires_at".to_string()))?,
        ),
        None => None,
    };
    let name = body.name;
    let created = blocking(state.backend.app(), move |app| {
        app.create_api_token_for_generation(&name, expires_at, user.credential_generation)
    })
    .await?;
    Ok(json_created(TokenCreatedDto {
        token_record: TokenDto::from_record(created.record),
        token: created.token,
    }))
}

pub async fn delete<B: Backend>(
    State(state): State<AppState<B>>,
    Extension(source): Extension<AuthSource>,
    Path(id): Path<String>,
) -> Result<Response, WebError> {
    require_session(source)?;
    let id = ApiTokenId(parse_id(&id)?);
    delete_ok(&state, "api token not found", move |app| {
        app.revoke_api_token(id)
    })
    .await
}
