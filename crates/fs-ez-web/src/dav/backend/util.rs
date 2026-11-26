//! Ponts asynchrones vers l'application et conversions d'erreurs DAV.

use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use dav_server::davpath::DavPath;
use dav_server::fs::FsError;
use fs_ez_app::App;
use time::OffsetDateTime;

use super::TEMP_DIR_MODE;
use crate::backend::blocking_join;
use crate::dav::logging;
use crate::dav::path::{self, Node, SelectionCache};

pub(super) async fn resolve(
    app: Arc<App>,
    path: &DavPath,
    cache: &SelectionCache,
) -> Result<Node, FsError> {
    let components = path::parse_components(path.as_bytes())?;
    let cache = cache.clone();
    blocking_join(app, move |app| {
        path::resolve_with(app, &components, Some(&cache))
    })
    .await
    .map_err(|error| {
        tracing::warn!(target: logging::TARGET, %error, "dav resolve task failed");
        FsError::GeneralFailure
    })?
}

pub(super) async fn call_app<T, F>(app: Arc<App>, f: F) -> Result<T, FsError>
where
    T: Send + 'static,
    F: FnOnce(&App) -> Result<T, fs_ez_app::AppError> + Send + 'static,
{
    blocking_join(app, f)
        .await
        .map_err(|error| {
            tracing::warn!(target: logging::TARGET, %error, "dav blocking task failed");
            FsError::GeneralFailure
        })?
        .map_err(path::fs_error)
}

/// Clés DAV des dossiers d'import sous la base canonique, feuille incluse.
pub(super) fn upload_dir_keys(
    components: &[String],
    base_len: usize,
) -> impl Iterator<Item = String> + '_ {
    (base_len.saturating_add(1)..=components.len()).map(move |end| components[..end].join("/"))
}

/// Parent créé sans chmod, le staging d'alias n'impose pas 0700 contrairement aux autres sites.
pub(super) async fn ensure_parent(path: &Path) -> Result<(), FsError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(io_error)?;
    }
    Ok(())
}

/// Variante 0700 du parent, chmod best-effort, échec ignoré comme aux appels d'origine.
pub(super) async fn ensure_private_parent(path: &Path) -> Result<(), FsError> {
    ensure_parent(path).await?;
    if let Some(parent) = path.parent() {
        let _ = tokio::fs::set_permissions(parent, std::fs::Permissions::from_mode(TEMP_DIR_MODE))
            .await;
    }
    Ok(())
}

/// Un rapport de suppression partielle rend le MOVE non atomique, refus global.
pub(super) fn ensure_no_failure(failed: usize) -> Result<(), FsError> {
    if failed > 0 {
        return Err(FsError::GeneralFailure);
    }
    Ok(())
}

pub(super) fn io_error(error: io::Error) -> FsError {
    match error.kind() {
        io::ErrorKind::NotFound => FsError::NotFound,
        io::ErrorKind::PermissionDenied => FsError::Forbidden,
        io::ErrorKind::AlreadyExists => FsError::Exists,
        _ => {
            tracing::warn!(target: logging::TARGET, %error, "dav io error");
            FsError::GeneralFailure
        }
    }
}

pub(super) fn system_time(value: OffsetDateTime) -> SystemTime {
    let nanos = value.unix_timestamp_nanos();
    if nanos >= 0 {
        let nanos = u64::try_from(nanos).unwrap_or(u64::MAX);
        SystemTime::UNIX_EPOCH + Duration::from_nanos(nanos)
    } else {
        let nanos = u64::try_from(nanos.unsigned_abs()).unwrap_or(u64::MAX);
        SystemTime::UNIX_EPOCH - Duration::from_nanos(nanos)
    }
}
