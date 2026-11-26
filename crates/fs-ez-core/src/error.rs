//! Erreurs de domaine, sans dépendance aux couches d'infrastructure.

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("invalid name: {0}")]
    InvalidName(String),
    #[error("invalid color: {0}")]
    InvalidColor(String),
    #[error("invalid fingerprint: {0}")]
    InvalidFingerprint(String),
    #[error("invalid filter: {0}")]
    InvalidFilter(String),
}
