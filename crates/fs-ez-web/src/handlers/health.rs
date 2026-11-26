//! Handlers minimaux : racine texte et sonde de santé sans accès base.

use axum::Json;
use axum::extract::State;
use serde::Serialize;

use crate::backend::Backend;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct Health {
    pub status: &'static str,
    pub version: &'static str,
}

/// Ne touche ni la DB ni la configuration : sonde bon marché pour le conteneur.
pub async fn health<B: Backend>(State(state): State<AppState<B>>) -> Json<Health> {
    Json(Health {
        status: "ok",
        version: state.backend.version(),
    })
}
