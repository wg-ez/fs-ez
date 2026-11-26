//! Initialisation du layout et réparation des chemins en base.

use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use fs_ez_core::{FileId, FileKind, FilePatch, FileRecord, Fingerprint};
use fs_ez_fs::{FileMeta, FsError, dir_lossy, display_lossy, file_name_lossy};

use crate::app::App;
use crate::error::AppError;

/// Profondeur maximale du parcours de recherche par empreinte.
const REPAIR_MAX_DEPTH: usize = 64;
/// Écritures accumulées avant une transaction de réparation.
const REPAIR_BATCH: usize = 500;

struct PendingRepair {
    id: FileId,
    patch: FilePatch,
    moved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitReport {
    pub root: PathBuf,
    pub db_path: PathBuf,
    pub files: PathBuf,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RepairReport {
    pub updated: u64,
    pub moved: u64,
    pub missing: u64,
}

impl App {
    /// Idempotent : crée le dossier d'état, les dossiers files et inbox, puis les réglages absents.
    pub fn init(&self, db_path: &Path) -> Result<InitReport, AppError> {
        let files = self.files_path()?;
        // `init_layout` résout les chemins absolus tels quels, les relatifs sous le root.
        self.root.init_layout(&files.to_string_lossy())?;
        self.root.create_public_dir(&self.inbox_path()?)?;
        let settings = self.settings()?;
        for (key, value) in settings.to_pairs() {
            if self.store.get_setting(&key)?.is_none() {
                self.store.set_setting(&key, &value)?;
            }
        }
        Ok(InitReport {
            root: self.root.path().to_path_buf(),
            db_path: absolute(db_path)?,
            files,
        })
    }

    /// Explicite, jamais appelé par un listing, ne touche jamais aux fichiers réels.
    pub fn repair(&self) -> Result<RepairReport, AppError> {
        let mut report = RepairReport::default();
        let mut pending: Vec<PendingRepair> = Vec::with_capacity(REPAIR_BATCH);
        for file in self.store.all_files()? {
            self.repair_file(&file, &mut pending, &mut report);
            if pending.len() >= REPAIR_BATCH {
                self.flush_repairs(&mut pending, &mut report);
            }
        }
        self.flush_repairs(&mut pending, &mut report);
        Ok(report)
    }

    /// Accumule les patchs ; une écriture groupée ratée retombe en unitaire.
    fn flush_repairs(&self, pending: &mut Vec<PendingRepair>, report: &mut RepairReport) {
        if pending.is_empty() {
            return;
        }
        let updates: Vec<(FileId, FilePatch)> = pending
            .iter()
            .map(|item| (item.id, item.patch.clone()))
            .collect();
        match self.store.update_files_batch(&updates) {
            Ok(_) => {
                for item in pending.drain(..) {
                    if item.moved {
                        report.moved += 1;
                    } else {
                        report.updated += 1;
                    }
                }
                return;
            }
            Err(error) => tracing::warn!(%error, "repair batch update failed, unit fallback"),
        }
        for item in pending.drain(..) {
            match self.store.update_file(item.id, &item.patch) {
                Ok(_) if item.moved => report.moved += 1,
                Ok(_) => report.updated += 1,
                Err(error) if item.moved => {
                    tracing::warn!(file = item.id.0, %error, "repair relocate failed");
                    report.missing += 1;
                }
                Err(error) => {
                    tracing::warn!(file = item.id.0, %error, "repair update failed");
                    report.missing += 1;
                }
            }
        }
    }

    fn repair_file(
        &self,
        file: &FileRecord,
        pending: &mut Vec<PendingRepair>,
        report: &mut RepairReport,
    ) {
        match self.fs.metadata(&file.path) {
            Ok(meta) if self.root.contains(&file.path) => {
                if meta.size != file.size || meta.mtime != file.mtime || meta.kind != file.kind {
                    pending.push(pending_repair(file.id, repair_patch(None, &meta), false));
                }
            }
            Ok(_) => report.missing += 1,
            Err(FsError::NotFound(_)) => match &file.fingerprint {
                Some(fingerprint) => {
                    if let Some(patch) = self.relocate(file, fingerprint) {
                        pending.push(pending_repair(file.id, patch, true));
                    } else {
                        report.missing += 1;
                    }
                }
                None => report.missing += 1,
            },
            Err(error) => {
                tracing::warn!(file = file.id.0, %error, "repair stat failed");
                report.missing += 1;
            }
        }
    }

    /// `None` si aucun candidat ; une lecture ratée compte `missing`.
    fn relocate(&self, file: &FileRecord, fingerprint: &Fingerprint) -> Option<FilePatch> {
        let found = self.search_by_fingerprint(fingerprint);
        let path = found.first()?;
        if found.len() > 1 {
            tracing::warn!(
                file = file.id.0,
                chosen = %display_lossy(path),
                others = found.len() - 1,
                "repair multiple fingerprint candidates"
            );
        }
        let meta = match self.fs.metadata(path) {
            Ok(meta) => meta,
            Err(error) => {
                tracing::warn!(file = file.id.0, %error, "repair candidate stat failed");
                return None;
            }
        };
        Some(repair_patch(Some(path), &meta))
    }

    /// Ordre lexical, liens symboliques jamais suivis, dossier d'état ignoré.
    fn search_by_fingerprint(&self, fingerprint: &Fingerprint) -> Vec<PathBuf> {
        let mut found = Vec::new();
        let mut stack = vec![(self.root.path().to_path_buf(), FileKind::Directory, 0usize)];
        while let Some((path, kind, depth)) = stack.pop() {
            match kind {
                FileKind::File => match self.fingerprints.fingerprint(&path, FileKind::File) {
                    Ok(candidate) if candidate == *fingerprint => {
                        found.push(path);
                        if found.len() > 1 {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(error) => {
                        tracing::debug!(path = %display_lossy(&path), %error, "fingerprint failed")
                    }
                },
                FileKind::Directory => {
                    if depth >= REPAIR_MAX_DEPTH {
                        continue;
                    }
                    let mut entries = match self.fs.read_dir(&path) {
                        Ok(entries) => entries,
                        Err(error) => {
                            tracing::debug!(path = %display_lossy(&path), %error, "walk failed");
                            continue;
                        }
                    };
                    entries.sort_by(|a, b| {
                        a.name
                            .as_os_str()
                            .as_bytes()
                            .cmp(b.name.as_os_str().as_bytes())
                    });
                    for entry in entries.into_iter().rev() {
                        let child = path.join(&entry.name);
                        if child.starts_with(self.root.state_dir())
                            || entry.kind == FileKind::Symlink
                        {
                            continue;
                        }
                        stack.push((child, entry.kind, depth + 1));
                    }
                }
                FileKind::Symlink => {}
            }
        }
        found
    }
}

fn pending_repair(id: FileId, patch: FilePatch, moved: bool) -> PendingRepair {
    PendingRepair { id, patch, moved }
}
fn repair_patch(path: Option<&Path>, meta: &FileMeta) -> FilePatch {
    FilePatch {
        path: path.map(Path::to_path_buf),
        path_display: path.map(display_lossy),
        name: path.map(|path| file_name_lossy(path).unwrap_or_default()),
        dir: path.map(|path| dir_lossy(path).unwrap_or_default()),
        size: Some(meta.size),
        mtime: Some(meta.mtime),
        kind: Some(meta.kind),
        fingerprint: None,
    }
}

fn absolute(path: &Path) -> Result<PathBuf, AppError> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    let cwd = std::env::current_dir()
        .map_err(|error| AppError::Fs(FsError::Io(format!("current dir: {error}"))))?;
    Ok(cwd.join(path))
}
