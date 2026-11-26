//! Configuration web : flags CLI > environnement > `fs-ez.toml` > réglages > défauts.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use fs_ez_app::load_file_config;
use fs_ez_fs::Root;

use crate::error::WebError;

const DEFAULT_HOST: &str = "0.0.0.0";
const DEFAULT_PORT: u16 = 8678;
/// 16 Gio.
const DEFAULT_MAX_UPLOAD: u64 = 16 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebConfig {
    pub host: String,
    pub port: u16,
    pub max_upload: u64,
    pub root: PathBuf,
    pub db_path: PathBuf,
    pub files: Option<PathBuf>,
}

impl WebConfig {
    /// `serve` transmet le root et la base du contexte CLI.
    pub fn from_env_with(root: &Path, db_path: &Path) -> Result<Self, WebError> {
        let map = env_map();
        Self::from_map_with(&map, root, db_path)
    }

    /// Variante testable : root explicite, base résolue par `fs-ez.toml` ou défaut.
    pub fn from_map(env: &HashMap<String, String>, root: &Path) -> Result<Self, WebError> {
        Self::resolve(env, root, None)
    }

    /// Variante testable : base explicite du contexte CLI prioritaire.
    pub fn from_map_with(
        env: &HashMap<String, String>,
        root: &Path,
        db_path: &Path,
    ) -> Result<Self, WebError> {
        Self::resolve(env, root, Some(db_path))
    }

    fn resolve(
        env: &HashMap<String, String>,
        root_path: &Path,
        db_override: Option<&Path>,
    ) -> Result<Self, WebError> {
        let root = Root::discover(Some(root_path))
            .map_err(|error| WebError::Config(format!("root: {error}")))?;
        let file = load_file_config(&root)
            .map_err(|error| WebError::Config(format!("fs-ez.toml: {error}")))?;

        let host = env_value(env, "HOST")
            .map(ToString::to_string)
            .or(file.host)
            .unwrap_or_else(|| DEFAULT_HOST.to_string());
        let port = match env_value(env, "PORT") {
            Some(value) => value
                .trim()
                .parse::<u16>()
                .map_err(|_| invalid("PORT", value))?,
            None => file.port.unwrap_or(DEFAULT_PORT),
        };
        let max_upload = match env_value(env, "MAX_UPLOAD") {
            Some(value) => match value.trim().parse::<u64>() {
                Ok(amount) if amount > 0 => amount,
                _ => return Err(invalid("MAX_UPLOAD", value)),
            },
            None => file.max_upload.unwrap_or(DEFAULT_MAX_UPLOAD),
        };
        let db_path = db_override
            .map(Path::to_path_buf)
            .or(file.db)
            .unwrap_or_else(|| root.default_db());
        let files = file.files;
        Ok(Self {
            host,
            port,
            max_upload,
            root: root.path().to_path_buf(),
            db_path,
            files,
        })
    }

    /// Flags `serve --host/--port` : prioritaires sur tout le reste.
    pub fn with_listen(mut self, host: Option<String>, port: Option<u16>) -> Self {
        if let Some(host) = host {
            self.host = host;
        }
        if let Some(port) = port {
            self.port = port;
        }
        self
    }
}

/// Variable d'environnement non vide : une valeur vide équivaut à l'absence.
fn env_value<'a>(env: &'a HashMap<String, String>, key: &str) -> Option<&'a str> {
    env.get(key)
        .map(String::as_str)
        .filter(|value| !value.is_empty())
}

fn env_map() -> HashMap<String, String> {
    std::env::vars_os()
        .map(|(key, value)| {
            (
                key.to_string_lossy().into_owned(),
                value.to_string_lossy().into_owned(),
            )
        })
        .collect()
}

fn invalid(variable: &str, value: &str) -> WebError {
    WebError::Config(format!("invalid {variable} value {value:?}"))
}

#[cfg(test)]
mod tests;
