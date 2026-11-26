//! Statut agrégé pour la CLI et l'API.

use std::fs;
use std::path::PathBuf;

use crate::app::App;
use crate::error::AppError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusReport {
    pub version: &'static str,
    pub root: PathBuf,
    pub db_path: PathBuf,
    pub db_size: u64,
    pub files: u64,
    pub untagged: u64,
    pub recent: u64,
    pub tags: u64,
    pub views: u64,
}

impl App {
    pub fn status(&self) -> Result<StatusReport, AppError> {
        let settings = self.settings()?;
        let db_path = self.db_path();
        Ok(StatusReport {
            version: env!("CARGO_PKG_VERSION"),
            root: self.root.path().to_path_buf(),
            db_size: db_size(&db_path),
            db_path,
            files: self.store.count_files()?,
            untagged: self.store.count_untagged()?,
            recent: self.store.count_tagged_since(settings.recent_days)?,
            tags: count(self.store.list_tags()?.len()),
            views: count(self.store.list_views()?.len()),
        })
    }

    /// Chemin figé par l'adaptateur sinon défaut du root.
    fn db_path(&self) -> PathBuf {
        if let Some(path) = &self.db_path {
            return path.clone();
        }
        self.root.default_db()
    }
}

fn count(len: usize) -> u64 {
    u64::try_from(len).unwrap_or(u64::MAX)
}

/// Somme DB, `-wal` et `-shm` ; fichiers absents ou illisibles ignorés.
fn db_size(db_path: &std::path::Path) -> u64 {
    ["", "-wal", "-shm"]
        .iter()
        .filter_map(|suffix| {
            let mut raw = db_path.as_os_str().to_os_string();
            raw.push(suffix);
            fs::metadata(PathBuf::from(raw)).ok()
        })
        .fold(0u64, |total, meta| total.saturating_add(meta.len()))
}
