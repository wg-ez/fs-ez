//! Couche `<root>/fs-ez.toml`, lue après la résolution du root.

use std::ffi::OsString;
use std::fs;
use std::path::{Component, PathBuf};

use fs_ez_core::Settings;
use fs_ez_fs::{FsError, Root, display_lossy};
use serde::Deserialize;

use crate::error::AppError;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FileConfig {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub max_upload: Option<u64>,
    pub db: Option<PathBuf>,
    pub files: Option<PathBuf>,
    pub inbox: Option<PathBuf>,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
}

pub const DEFAULT_UID: u32 = 1000;
pub const DEFAULT_GID: u32 = 1000;

/// Première source `Some` dans l'ordre, `default` sinon.
pub fn first_some<T>(
    sources: impl IntoIterator<Item = Option<T>>,
    default: impl FnOnce() -> T,
) -> T {
    sources.into_iter().flatten().next().unwrap_or_else(default)
}

/// Variable d'environnement non vide, `None` si absente ou vide.
pub fn env_non_empty(variable: &str) -> Option<OsString> {
    std::env::var_os(variable).filter(|value| !value.is_empty())
}

/// Propriétaire appliqué aux fichiers confiés à fs-ez.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileOwner {
    pub uid: u32,
    pub gid: u32,
}

impl Default for FileOwner {
    fn default() -> Self {
        Self {
            uid: DEFAULT_UID,
            gid: DEFAULT_GID,
        }
    }
}

/// Priorité env `PUID`/`PGID` → `fs-ez.toml` → 1000:1000.
pub fn file_owner(root: &Root) -> Result<FileOwner, AppError> {
    let config = load_file_config(root)?;
    Ok(FileOwner {
        uid: resolve_id("PUID", config.uid, DEFAULT_UID)?,
        gid: resolve_id("PGID", config.gid, DEFAULT_GID)?,
    })
}

fn resolve_id(variable: &str, config: Option<u32>, default: u32) -> Result<u32, AppError> {
    let Some(value) = env_non_empty(variable) else {
        return Ok(config.unwrap_or(default));
    };
    value
        .to_string_lossy()
        .trim()
        .parse::<u32>()
        .map_err(|_| AppError::Invalid(format!("invalid {variable} value {value:?}")))
}

pub fn load_file_config(root: &Root) -> Result<FileConfig, AppError> {
    let path = root.path().join("fs-ez.toml");
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(FileConfig::default());
        }
        Err(error) => {
            return Err(invalid(&path, &error.to_string()));
        }
    };
    let mut config: FileConfig =
        toml::from_str(&text).map_err(|error| invalid(&path, &error.to_string()))?;
    config.db = config.db.map(|path| resolve(root, path)).transpose()?;
    config.files = config.files.map(|path| resolve(root, path)).transpose()?;
    config.inbox = config.inbox.map(|path| resolve(root, path)).transpose()?;
    Ok(config)
}

/// Priorité `fs-ez.toml` → settings → défaut du `Root`, settings lus seulement si besoin.
pub(crate) fn resolve_io_path(
    root: &Root,
    settings: impl FnOnce() -> Result<Settings, AppError>,
    toml_field: impl FnOnce(&FileConfig) -> Option<PathBuf>,
    settings_field: impl FnOnce(&Settings) -> String,
    default: impl FnOnce(&Root, &str) -> Result<PathBuf, FsError>,
) -> Result<PathBuf, AppError> {
    if let Some(path) = toml_field(&load_file_config(root)?) {
        return Ok(path);
    }
    let settings = settings()?;
    Ok(default(root, &settings_field(&settings))?)
}

fn invalid(path: &std::path::Path, message: &str) -> AppError {
    AppError::Invalid(format!("{}: {message}", display_lossy(path)))
}

fn resolve(root: &Root, path: PathBuf) -> Result<PathBuf, AppError> {
    if path.as_os_str().is_empty() {
        return Err(AppError::Invalid("empty path in fs-ez.toml".to_string()));
    }
    if path.is_absolute() {
        return Ok(path);
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(AppError::Invalid(format!(
            "path escapes root in fs-ez.toml: {}",
            display_lossy(&path)
        )));
    }
    Ok(root.path().join(path))
}
