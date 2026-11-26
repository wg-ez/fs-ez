//! Erreurs uniformes de la CLI.

use fs_ez_app::AppError;

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("usage error: {0}")]
    Usage(String),
    #[error("{0}")]
    App(#[from] AppError),
    #[error("io error: {0}")]
    Io(String),
    #[error("json error: {0}")]
    Json(String),
}

#[cfg(test)]
mod tests;
