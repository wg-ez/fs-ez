//! Import par dépôt dans le dossier files : empreinte et doublons.

pub(crate) mod dedup;
mod inbox;
mod index;
mod naming;
mod placement;
mod scan;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use fs_ez_core::FileRecord;
use fs_ez_fs::{FsError, display_lossy};
use serde::Serialize;

use crate::app::App;
use crate::error::AppError;

pub use naming::{is_temporary, sanitize_filename};

#[derive(Debug, Serialize)]
pub enum LibraryOutcome {
    Indexed(FileRecord),
    Duplicate { existing: FileRecord },
}

#[derive(Debug, Default)]
pub struct ImportReport {
    pub imported: Vec<FileRecord>,
    pub duplicates: Vec<(PathBuf, FileRecord)>,
    pub skipped: Vec<(PathBuf, String)>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImportCounts {
    pub imported: usize,
    pub duplicates: usize,
    pub skipped: usize,
}

impl ImportReport {
    /// Volumes du rapport, sans les entrées détaillées.
    pub fn counts(&self) -> ImportCounts {
        ImportCounts {
            imported: self.imported.len(),
            duplicates: self.duplicates.len(),
            skipped: self.skipped.len(),
        }
    }
}

impl LibraryOutcome {
    /// Statut stable exposé par l'API web : `imported` ou `duplicate`.
    pub fn status(&self) -> &'static str {
        match self {
            Self::Indexed(_) => "imported",
            Self::Duplicate { .. } => "duplicate",
        }
    }
}

impl App {
    /// Priorité `fs-ez.toml` → `settings.files_dir`.
    pub fn files_path(&self) -> Result<PathBuf, AppError> {
        crate::config::resolve_io_path(
            &self.root,
            || self.settings(),
            |config| config.files.clone(),
            |settings| settings.files_dir.clone(),
            |root, value| root.files(value),
        )
    }

    /// Sous le dossier d'état : volume séparé de `files/`, l'import copie alors le fichier.
    pub fn upload_tmp_dir(&self) -> PathBuf {
        self.root.state_dir().join("tmp")
    }

    /// Préfixe virtuel de tout import inbox : `inbox` puis date locale du jour.
    pub fn inbox_virtual_prefix(&self) -> Vec<String> {
        vec![
            "inbox".to_string(),
            fs_ez_core::datetime::now_local().date().to_string(),
        ]
    }

    /// Priorité `fs-ez.toml` champ `inbox` → `<root>/inbox`.
    pub fn inbox_path(&self) -> Result<PathBuf, AppError> {
        if let Some(path) = crate::config::load_file_config(&self.root)?.inbox {
            return Ok(path);
        }
        Ok(self.root.files("inbox")?)
    }

    /// Suppression best-effort, échec journalisé sans interrompre l'appelant.
    fn remove_quiet(&self, path: &Path, label: &str) {
        if let Err(error) = self.fs.remove_file(path) {
            tracing::warn!(path = %display_lossy(path), %error, "{label}");
        }
    }
}

/// Toute variation de taille ou de mtime invalide l'empreinte précalculée.
fn metadata_changed(before: &fs_ez_fs::FileMeta, after: &fs_ez_fs::FileMeta) -> bool {
    before.size != after.size || before.mtime != after.mtime
}

/// Nom de fichier lossy, repli `fichier` pour une source sans nom.
fn import_name(path: &Path) -> String {
    fs_ez_fs::file_name_lossy(path).unwrap_or_else(|| "fichier".to_string())
}

/// Index par empreinte, premier enregistrement gagnant, entrées sans empreinte ignorées.
fn by_fingerprint(records: Vec<FileRecord>) -> HashMap<String, FileRecord> {
    let mut map: HashMap<String, FileRecord> = HashMap::new();
    for record in records {
        if let Some(fingerprint) = &record.fingerprint {
            let key = fingerprint.as_str().to_string();
            map.entry(key).or_insert(record);
        }
    }
    map
}

/// Composants relatifs à la racine, ordre racine → feuille, tronqués à `max_depth`.
fn relative_components(root: &Path, path: &Path, max_depth: usize) -> Vec<String> {
    let Ok(relative) = path.strip_prefix(root) else {
        return Vec::new();
    };
    relative
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
            _ => None,
        })
        .take(max_depth)
        .collect()
}
