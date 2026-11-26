//! Erreurs web uniformes : tout corps JSON porte `schema`, `error` et `code`.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use fs_ez_app::AppError;
use serde_json::json;

use crate::api::errors::SCHEMA_VERSION;

#[derive(Debug, thiserror::Error)]
pub enum WebError {
    #[error("{0}")]
    Config(String),
    #[error("{0}")]
    Internal(String),
    #[error("{0}")]
    NotFound(String),
    #[error("unauthorized")]
    Unauthorized,
    // Guard : message dédié, code `unauthorized` partagé avec `Unauthorized`.
    #[error("authentication required")]
    AuthenticationRequired,
    #[error("forbidden")]
    Forbidden,
    // Guard CSRF : message dédié, code `csrf` distinct de `forbidden`.
    #[error("cross-site request rejected")]
    Csrf,
    #[error("{0}")]
    BadRequest(String),
    #[error("payload too large")]
    PayloadTooLarge,
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Db(String),
    #[error("{0}")]
    Upstream(String),
}

impl WebError {
    /// Statut et code stables : un seul match pour les deux projections.
    fn meta(&self) -> (StatusCode, &'static str) {
        match self {
            Self::Config(_) => (StatusCode::INTERNAL_SERVER_ERROR, "config"),
            Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
            Self::NotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
            Self::Unauthorized | Self::AuthenticationRequired => {
                (StatusCode::UNAUTHORIZED, "unauthorized")
            }
            Self::Forbidden => (StatusCode::FORBIDDEN, "forbidden"),
            Self::Csrf => (StatusCode::FORBIDDEN, "csrf"),
            Self::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request"),
            Self::PayloadTooLarge => (StatusCode::PAYLOAD_TOO_LARGE, "payload_too_large"),
            Self::Conflict(_) => (StatusCode::CONFLICT, "conflict"),
            Self::Db(_) => (StatusCode::INTERNAL_SERVER_ERROR, "db"),
            Self::Upstream(_) => (StatusCode::BAD_GATEWAY, "upstream"),
        }
    }

    pub fn status(&self) -> StatusCode {
        self.meta().0
    }

    pub fn code(&self) -> &'static str {
        self.meta().1
    }
}

impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        let (status, code) = self.meta();
        // Le détail reste dans les logs : jamais de chemin serveur ni de SQL dans le corps.
        if status.is_server_error() {
            tracing::error!(error = %self, code, "web error");
        }
        let message = if status.is_server_error() {
            "internal server error".to_string()
        } else {
            self.to_string()
        };
        let body = json!({ "schema": SCHEMA_VERSION, "error": message, "code": code });
        (status, Json(body)).into_response()
    }
}

impl From<AppError> for WebError {
    fn from(error: AppError) -> Self {
        match error {
            AppError::NotFound(message) => Self::NotFound(message),
            AppError::Conflict(message) => Self::Conflict(message),
            AppError::Invalid(message) => Self::BadRequest(message),
            AppError::Core(error) => Self::BadRequest(error.to_string()),
            AppError::Query(error) => Self::BadRequest(error.to_string()),
            AppError::Store(fs_ez_store::StoreError::Invalid(message)) => Self::BadRequest(message),
            AppError::Store(fs_ez_store::StoreError::NotFound(what)) => {
                Self::NotFound(what.to_string())
            }
            AppError::Store(fs_ez_store::StoreError::Conflict(message)) => Self::Conflict(message),
            other => Self::Internal(other.to_string()),
        }
    }
}
