//! Erreurs de l'abstraction FS.

use std::io;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum FsError {
    #[error("io error: {0}")]
    Io(String),
    #[error("path is outside root: {0}")]
    OutsideRoot(String),
    #[error("invalid path: {0}")]
    Invalid(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("already exists: {0}")]
    AlreadyExists(String),
    #[error("cross-device move: {0}")]
    CrossDevice(String),
}

impl FsError {
    pub(crate) fn from_io(path: &Path, err: io::Error) -> Self {
        let display = crate::path::display_lossy(path);
        if err.kind() == io::ErrorKind::NotFound {
            Self::NotFound(display)
        } else if err.kind() == io::ErrorKind::AlreadyExists {
            Self::AlreadyExists(display)
        } else if err.kind() == io::ErrorKind::CrossesDevices {
            Self::CrossDevice(display)
        } else {
            Self::Io(format!("{display}: {err}"))
        }
    }
}
