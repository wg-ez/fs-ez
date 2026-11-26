//! Chemins non-UTF8 et normalisation des chemins relatifs.

use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use crate::error::FsError;

/// Octets Unix bruts d'un chemin, sans conversion UTF-8.
pub fn path_bytes(path: &Path) -> &[u8] {
    path.as_os_str().as_bytes()
}

/// Reconstruit un chemin depuis ses octets Unix bruts.
pub fn path_from_bytes(bytes: Vec<u8>) -> PathBuf {
    PathBuf::from(OsString::from_vec(bytes))
}

pub fn display_lossy(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub fn file_name_lossy(path: &Path) -> Option<String> {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

pub fn dir_lossy(path: &Path) -> Option<String> {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(|parent| parent.to_string_lossy().into_owned())
}

/// Chemin relatif sûr : ni absolu, ni `..`, ni `.`, ni composant vide, ni NUL.
pub fn normalize_relative(raw: &str) -> Result<PathBuf, FsError> {
    fs_ez_core::normalize_relative_path(raw)
        .ok_or_else(|| FsError::Invalid(format!("invalid relative path: {raw:?}")))
}

#[cfg(test)]
mod tests;
