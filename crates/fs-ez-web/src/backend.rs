//! Trait `Backend` minimal et exécution des appels bloquants hors runtime async.

use std::sync::Arc;

use fs_ez_app::{App, AppError};

use crate::error::WebError;

pub trait Backend: Send + Sync + 'static {
    fn app(&self) -> Arc<App>;
    fn version(&self) -> &'static str;
}

pub struct AppBackend {
    app: Arc<App>,
}

impl AppBackend {
    pub fn new(app: Arc<App>) -> Self {
        Self { app }
    }
}

impl Backend for AppBackend {
    fn app(&self) -> Arc<App> {
        Arc::clone(&self.app)
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }
}

/// `spawn_blocking` obligatoire : `App` et le store SQLite sont synchrones.
pub async fn blocking<T, F>(app: Arc<App>, f: F) -> Result<T, WebError>
where
    T: Send + 'static,
    F: FnOnce(&App) -> Result<T, AppError> + Send + 'static,
{
    blocking_join(app, f)
        .await
        .map_err(|error| WebError::Internal(format!("blocking task failed: {error}")))?
        .map_err(WebError::from)
}

/// Variante brute : l'appelant mappe `JoinError` et l'erreur applicative.
pub async fn blocking_join<T, E, F>(
    app: Arc<App>,
    f: F,
) -> Result<Result<T, E>, tokio::task::JoinError>
where
    T: Send + 'static,
    E: Send + 'static,
    F: FnOnce(&App) -> Result<T, E> + Send + 'static,
{
    tokio::task::spawn_blocking(move || f(&app)).await
}
