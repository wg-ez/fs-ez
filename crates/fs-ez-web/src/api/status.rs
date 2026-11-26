//! API JSON du statut agrégé : compteurs de la fiche 17B, aucun secret.

use axum::extract::State;
use axum::response::Response;

use crate::backend::{Backend, blocking};
use crate::error::WebError;
use crate::state::AppState;

use super::dto::StatusDto;
use super::errors::json_ok;

pub async fn get<B: Backend>(State(state): State<AppState<B>>) -> Result<Response, WebError> {
    let report = blocking(state.backend.app(), |app| app.status()).await?;
    Ok(json_ok(StatusDto::from_report(report)))
}
