//! API JSON de scan de l'inbox.

use axum::extract::State;
use axum::response::Response;

use crate::backend::{Backend, blocking};
use crate::error::WebError;
use crate::state::{AppState, Event};

use super::errors::json_ok;

#[derive(Debug, serde::Serialize)]
struct ScanDto {
    imported: usize,
    duplicates: usize,
    skipped: usize,
}

pub async fn scan<B: Backend>(State(state): State<AppState<B>>) -> Result<Response, WebError> {
    let counts = blocking(state.backend.app(), |app| app.scan_inbox()).await?;
    let _ = state.events.send(Event::InboxChanged);
    Ok(json_ok(ScanDto {
        imported: counts.imported,
        duplicates: counts.duplicates,
        skipped: counts.skipped,
    }))
}
