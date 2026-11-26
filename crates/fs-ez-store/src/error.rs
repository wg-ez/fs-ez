//! Erreurs des ports de persistance, sans fuite de type SQLite.

use fs_ez_core::CoreError;

/// Conflit quand la generation de credential a change avant l'insertion d'un jeton.
pub const STALE_CREDENTIAL_GENERATION: &str = "credential generation changed";

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Db(String),
    #[error("not found: {0}")]
    NotFound(&'static str),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("invalid: {0}")]
    Invalid(String),
    #[error("io error: {0}")]
    Io(String),
}

impl From<CoreError> for StoreError {
    fn from(error: CoreError) -> Self {
        Self::Invalid(error.to_string())
    }
}

pub type StoreResult<T> = Result<T, StoreError>;

#[cfg(test)]
mod tests;
