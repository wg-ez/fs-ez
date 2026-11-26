//! Erreurs des cas d'usage, sans fuite des dépendances d'infrastructure.

use std::io;
use std::path::Path;

use fs_ez_core::CoreError;
use fs_ez_fingerprint::FingerprintError;
use fs_ez_fs::{FsError, display_lossy};
use fs_ez_query::QueryError;
use fs_ez_store::StoreError;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Store(#[from] StoreError),
    #[error("{0}")]
    Fs(#[from] FsError),
    #[error("{0}")]
    Fingerprint(#[from] FingerprintError),
    #[error("{0}")]
    Core(#[from] CoreError),
    #[error("{0}")]
    Query(#[from] QueryError),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("invalid: {0}")]
    Invalid(String),
}

/// Erreur d'E/S attribuée au chemin fautif, affichage lossy du chemin.
pub(crate) fn io_error(path: &Path, error: io::Error) -> AppError {
    AppError::Fs(FsError::Io(format!("{}: {error}", display_lossy(path))))
}
