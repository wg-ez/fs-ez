use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

use fs_ez_core::{FileId, FileKind, FileRecord};
use fs_ez_fs::{Fs, FsError, display_lossy};

use crate::app::App;
use crate::error::AppError;

/// Sous ce seuil le surcoût des threads dépasse le gain de `unlink`.
const PARALLEL_UNLINK_MIN: usize = 256;
/// Borné : le disque et le système de fichiers ne profitent pas de plus.
const UNLINK_WORKERS: usize = 8;

/// Suppression best-effort : le disque et SQLite ne partagent pas de transaction.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeleteReport {
    pub deleted: usize,
    pub failed: usize,
}

impl App {
    /// Supprime du disque puis de la base, idempotent sur un fichier déjà absent.
    pub fn delete_files(&self, ids: &[FileId]) -> Result<DeleteReport, AppError> {
        if ids.is_empty() {
            return Ok(DeleteReport::default());
        }
        self.delete_resolved_files(&self.store.get_files_by_ids(ids)?)
    }

    /// Variante DAV : enregistrements déjà résolus, aucune relecture `get_file`.
    pub fn delete_resolved_files(&self, records: &[FileRecord]) -> Result<DeleteReport, AppError> {
        let mut report = DeleteReport::default();
        let mut seen = HashSet::with_capacity(records.len());
        let mut parents: HashMap<PathBuf, PathBuf> = HashMap::new();
        let mut entries = Vec::with_capacity(records.len());
        for record in records {
            if !seen.insert(record.id) {
                continue;
            }
            if record.kind == FileKind::Directory {
                report.failed = report.failed.saturating_add(1);
                continue;
            }
            let path = match self.confine_entry_path_cached(&record.path, &mut parents) {
                Ok(path) => path,
                Err(error) => {
                    tracing::warn!(file = record.id.0, %error, "delete path rejected");
                    report.failed = report.failed.saturating_add(1);
                    continue;
                }
            };
            entries.push((record.id, path));
        }
        let mut removable = Vec::with_capacity(entries.len());
        for (id, outcome) in remove_entries(self.fs.as_ref(), entries) {
            match outcome {
                Ok(()) | Err(FsError::NotFound(_)) => removable.push(id),
                Err(error) => {
                    tracing::warn!(file = id.0, %error, "delete failed");
                    report.failed = report.failed.saturating_add(1);
                }
            }
        }
        if removable.is_empty() {
            return Ok(report);
        }
        match self.store.delete_files_batch(&removable) {
            Ok(_) => report.deleted = report.deleted.saturating_add(removable.len()),
            Err(error) => {
                tracing::warn!(%error, "batch index delete failed after file removal");
                for id in removable {
                    match self.store.delete_file(id) {
                        Ok(_) => report.deleted = report.deleted.saturating_add(1),
                        Err(error) => {
                            tracing::warn!(file = id.0, %error, "index delete failed after file removal");
                            report.failed = report.failed.saturating_add(1);
                        }
                    }
                }
            }
        }
        Ok(report)
    }

    /// Confine en canonicalisant chaque parent une seule fois, les shards se répètent.
    fn confine_entry_path_cached(
        &self,
        path: &Path,
        parents: &mut HashMap<PathBuf, PathBuf>,
    ) -> Result<PathBuf, AppError> {
        if path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(FsError::OutsideRoot(display_lossy(path)).into());
        }
        let file_name = path.file_name().ok_or_else(|| {
            AppError::Invalid(format!("missing file name: {}", display_lossy(path)))
        })?;
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let canonical_parent = match parents.get(parent) {
            Some(canonical) => canonical.clone(),
            None => {
                let canonical = self.fs.canonicalize(parent)?;
                parents.insert(parent.to_path_buf(), canonical.clone());
                canonical
            }
        };
        let joined = canonical_parent.join(file_name);
        if !self.root.contains(&joined) {
            return Err(FsError::OutsideRoot(display_lossy(path)).into());
        }
        self.ensure_outside_state(&joined)?;
        Ok(joined)
    }
}

/// `unlink` borné en parallèle sur les gros lots, ordre des résultats non garanti.
fn remove_entries(
    fs: &dyn Fs,
    entries: Vec<(FileId, PathBuf)>,
) -> Vec<(FileId, Result<(), FsError>)> {
    if entries.len() < PARALLEL_UNLINK_MIN {
        return entries
            .into_iter()
            .map(|(id, path)| (id, fs.remove_file(&path)))
            .collect();
    }
    let workers = UNLINK_WORKERS.min(entries.len());
    let chunk_size = entries.len().div_ceil(workers);
    let mut outcomes = Vec::with_capacity(entries.len());
    std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(workers);
        for chunk in entries.chunks(chunk_size) {
            let ids: Vec<FileId> = chunk.iter().map(|(id, _)| *id).collect();
            handles.push((
                ids,
                scope.spawn(move || {
                    chunk
                        .iter()
                        .map(|(id, path)| (*id, fs.remove_file(path)))
                        .collect::<Vec<_>>()
                }),
            ));
        }
        for (ids, handle) in handles {
            match handle.join() {
                Ok(chunk) => outcomes.extend(chunk),
                Err(_) => outcomes.extend(
                    ids.into_iter()
                        .map(|id| (id, Err(FsError::Io("unlink worker panicked".to_string())))),
                ),
            }
        }
    });
    outcomes
}
