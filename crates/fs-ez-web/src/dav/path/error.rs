use fs_ez_app::AppError;

use super::super::logging;

pub(super) fn store_error(error: fs_ez_store::StoreError) -> dav_server::fs::FsError {
    fs_error(AppError::Store(error))
}

/// FsError stable sans fuite de détail serveur, journalisé en debug.
pub fn fs_error(error: AppError) -> dav_server::fs::FsError {
    use dav_server::fs::FsError;
    match &error {
        AppError::NotFound(_) => FsError::NotFound,
        AppError::Store(fs_ez_store::StoreError::NotFound(_)) => FsError::NotFound,
        AppError::Invalid(_) | AppError::Conflict(_) | AppError::Core(_) => FsError::Forbidden,
        _ => {
            tracing::warn!(target: logging::TARGET, %error, "dav backend error");
            FsError::GeneralFailure
        }
    }
}
