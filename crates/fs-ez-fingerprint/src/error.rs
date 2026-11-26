//! Erreurs du calcul d'empreinte.

use std::io;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum FingerprintError {
    #[error("io error: {0}")]
    Io(String),
    #[error("unsupported file kind: {0}")]
    Unsupported(String),
    #[error("invalid sampling configuration")]
    Config,
}

impl FingerprintError {
    /// Inclut le chemin dans le message : un échec d'empreinte doit rester localisable.
    pub(crate) fn io(path: &Path, error: io::Error) -> Self {
        Self::Io(format!("{}: {error}", path.display()))
    }

    pub(crate) fn unsupported(path: &Path) -> Self {
        Self::Unsupported(path.display().to_string())
    }
}
