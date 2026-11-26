//! Enveloppe JSON versionnée, extraction des corps bornés et validateurs communs.

use axum::Json;
use axum::extract::{FromRequest, Request};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use fs_ez_app::{App, AppError};
use fs_ez_core::TagColor;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::backend::{Backend, blocking};
use crate::error::WebError;
use crate::forms::parse_positive_id;
use crate::state::AppState;

/// Version du schéma JSON portée par toutes les réponses, API et erreurs.
pub const SCHEMA_VERSION: u8 = 1;
/// Corps JSON maximal accepté par les routes de cette fiche.
pub const API_BODY_LIMIT: usize = 2 * 1024 * 1024;
/// Taille de page par défaut de l'API.
const DEFAULT_LIMIT: usize = 100;
/// Borne des positions, marge pour l'auto-incrément des vues.
pub const MAX_POSITION: i64 = 1_000_000_000;

#[derive(Debug, Serialize)]
struct Envelope<T> {
    schema: u8,
    #[serde(flatten)]
    body: T,
}

fn envelope<T: Serialize>(status: StatusCode, body: T) -> Response {
    let payload = Envelope {
        schema: SCHEMA_VERSION,
        body,
    };
    (status, Json(payload)).into_response()
}

pub fn json_ok<T: Serialize>(body: T) -> Response {
    envelope(StatusCode::OK, body)
}

pub fn json_created<T: Serialize>(body: T) -> Response {
    envelope(StatusCode::CREATED, body)
}

/// Suppression effective → `{ "ok": true }`, sinon 404 avec le message de la ressource.
pub async fn delete_ok<B: Backend>(
    state: &AppState<B>,
    message: &str,
    delete: impl FnOnce(&App) -> Result<bool, AppError> + Send + 'static,
) -> Result<Response, WebError> {
    if blocking(state.backend.app(), delete).await? {
        Ok(json_ok(serde_json::json!({ "ok": true })))
    } else {
        Err(WebError::NotFound(message.to_string()))
    }
}

/// JSON désérialisé en un seul passage : toute erreur devient un `WebError` JSON.
pub struct ApiJson<T>(pub T);

impl<T: DeserializeOwned, S: Send + Sync> FromRequest<S> for ApiJson<T> {
    type Rejection = WebError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let bytes = axum::body::Bytes::from_request(req, state)
            .await
            .map_err(|rejection| match rejection.status() {
                StatusCode::PAYLOAD_TOO_LARGE => WebError::PayloadTooLarge,
                _ => WebError::BadRequest("invalid request body".to_string()),
            })?;
        if bytes.is_empty() {
            return Err(WebError::BadRequest("JSON body required".to_string()));
        }
        serde_json::from_slice(&bytes)
            .map(Self)
            .map_err(|error| WebError::BadRequest(format!("invalid JSON: {error}")))
    }
}

/// Identifiant positif de ressource : sinon 400, jamais une erreur de routage en texte.
pub fn parse_id(raw: &str) -> Result<i64, WebError> {
    // Les segments d'URL ne sont pas trimés : un espace reste invalide ici.
    parse_positive_id(raw)
        .filter(|_| raw.trim() == raw)
        .ok_or_else(|| WebError::BadRequest("invalid resource id".to_string()))
}

pub fn parse_offset(raw: Option<&str>) -> Result<usize, WebError> {
    match crate::query_params::trimmed_non_empty(raw) {
        Some(raw) => crate::query_params::parse_offset_number(raw),
        None => Ok(0),
    }
}

pub fn parse_limit(raw: Option<&str>) -> Result<usize, WebError> {
    let Some(raw) = crate::query_params::trimmed_non_empty(raw) else {
        return Ok(DEFAULT_LIMIT);
    };
    let limit = crate::query_params::parse_limit_number(raw)?;
    fs_ez_store::validate_page_limit(limit).map_err(|error| match error {
        fs_ez_store::StoreError::Invalid(message) => WebError::BadRequest(message),
        _ => WebError::BadRequest("invalid limit".to_string()),
    })?;
    Ok(limit)
}

pub(crate) use crate::forms::parse_optional_color;

/// Serde aplatit `null` vers `None` sur un `Option<Option<_>>`, ce visiteur force `Some(None)`.
pub fn deserialize_double_option<'de, D>(
    deserializer: D,
) -> Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer).map(Some)
}

/// Tri-état PATCH : absent = inchangé, `null` = effacé, valeur = validée.
pub fn parse_patch_color(
    raw: Option<&Option<String>>,
) -> Result<Option<Option<TagColor>>, WebError> {
    raw.map(|raw| parse_optional_color(raw.as_deref()))
        .transpose()
}

/// Actions `assign`/`add` acceptées, `remove` refusé selon la ressource.
pub fn check_assign_action(action: &str, resource: &str) -> Result<(), WebError> {
    match action {
        "assign" | "add" => Ok(()),
        "remove" => Err(WebError::BadRequest(format!(
            "a {resource} belongs to exactly one group; move it or delete it"
        ))),
        other => Err(WebError::BadRequest(format!("unknown action {other:?}"))),
    }
}

/// `champ` ou `champ:asc|desc`, direction ascendante par défaut, politique commune à l'UI.
pub fn parse_sort(raw: Option<&str>) -> Result<Option<fs_ez_core::Sort>, WebError> {
    match crate::query_params::trimmed_non_empty(raw) {
        Some(raw) => crate::query_params::parse_sort_arg(raw).map(Some),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests;
