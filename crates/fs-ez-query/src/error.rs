//! Erreurs du langage de requêtes.

use fs_ez_core::CoreError;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum QueryError {
    #[error("unexpected end of query")]
    UnexpectedEnd,
    #[error("unexpected token `{0}` at position {1}")]
    UnexpectedToken(String, usize),
    #[error("invalid tag or value: {0}")]
    InvalidName(String),
    #[error("empty query")]
    Empty,
    #[error("query too complex")]
    TooComplex,
}

impl QueryError {
    /// Traduction commune d'une erreur du noyau, le nom invalide garde son message.
    pub(crate) fn from_core(error: CoreError) -> Self {
        match error {
            CoreError::InvalidName(name) => Self::InvalidName(name),
            other => Self::InvalidName(other.to_string()),
        }
    }
}
