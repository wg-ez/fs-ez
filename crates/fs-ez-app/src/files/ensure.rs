use std::path::Path;

use fs_ez_core::{FileId, FileKind, FilePatch, FileRecord, NewFile};
use fs_ez_fs::{dir_lossy, display_lossy, file_name_lossy};

use crate::app::App;
use crate::error::AppError;
use crate::import::dedup::FingerprintResolution;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnsureOptions {
    pub force: bool,
}

impl App {
    pub fn ensure_file(&self, path: &Path, opts: EnsureOptions) -> Result<FileRecord, AppError> {
        let existing_meta = match self.fs.metadata(path) {
            Ok(meta) => Some(meta),
            Err(fs_ez_fs::FsError::NotFound(_)) => None,
            Err(error) => return Err(error.into()),
        };
        let (confined, meta, mut fingerprint) = match existing_meta {
            Some(meta) => {
                if meta.is_special {
                    return Err(AppError::Invalid(format!(
                        "special file: {}",
                        display_lossy(path)
                    )));
                }
                let confined = self.root.confine_existing(path)?;
                let fingerprint = if meta.kind == FileKind::File {
                    Some(self.fingerprints.fingerprint(&confined, meta.kind)?)
                } else {
                    None
                };
                (confined, meta, fingerprint)
            }
            None if opts.force => {
                let confined = self.root.confine_new(path)?;
                let meta = fs_ez_fs::FileMeta {
                    size: 0,
                    mtime: self.now(),
                    kind: FileKind::File,
                    is_special: false,
                };
                (confined, meta, None)
            }
            None => return Err(AppError::NotFound(display_lossy(path))),
        };
        let existing_by_path = self.store.find_file_by_path(&confined)?;
        if let Some(value) = fingerprint.clone() {
            match self.resolve_fingerprint(
                &confined,
                value,
                self.fingerprint_covers_file(meta.size),
            )? {
                FingerprintResolution::Available(resolved) => fingerprint = Some(resolved),
                FingerprintResolution::Duplicate(other) => {
                    return Err(duplicate_conflict(&other));
                }
            }
        }
        let new = new_file_from_meta(&confined, &meta, fingerprint.clone());
        if let Some(existing) = existing_by_path {
            return self.refresh_file(existing.id, &new);
        }
        match self.store.insert_file(&new) {
            Ok(record) => Ok(record),
            // Course entre deux writers : la ligne gagnante est relue puis rafraîchie.
            Err(fs_ez_store::StoreError::Conflict(_)) => {
                match self.store.find_file_by_path(&confined)? {
                    Some(existing) => self.refresh_file(existing.id, &new),
                    None => Err(AppError::Conflict(format!(
                        "conflicting file at {}",
                        display_lossy(&confined)
                    ))),
                }
            }
            Err(error) => Err(error.into()),
        }
    }

    fn refresh_file(&self, id: FileId, new: &NewFile) -> Result<FileRecord, AppError> {
        let mut patch = patch_from_new(new);
        patch.fingerprint = match new.kind {
            FileKind::File => Some(new.fingerprint.clone()),
            _ => None,
        };
        Ok(self.store.update_file(id, &patch)?)
    }
}

/// Conflit stable d'empreinte déjà connue sous un autre chemin.
fn duplicate_conflict(other: &FileRecord) -> AppError {
    AppError::Conflict(format!(
        "duplicate fingerprint, already known as {}",
        other.path_display
    ))
}

pub(crate) fn new_file_from_meta(
    path: &Path,
    meta: &fs_ez_fs::FileMeta,
    fingerprint: Option<fs_ez_core::Fingerprint>,
) -> NewFile {
    NewFile {
        path_display: display_lossy(path),
        name: file_name_lossy(path).unwrap_or_default(),
        dir: dir_lossy(path).unwrap_or_default(),
        path: path.to_path_buf(),
        size: meta.size,
        mtime: meta.mtime,
        kind: meta.kind,
        fingerprint,
    }
}

pub(crate) fn patch_from_new(new: &NewFile) -> FilePatch {
    FilePatch {
        path: Some(new.path.clone()),
        path_display: Some(new.path_display.clone()),
        name: Some(new.name.clone()),
        dir: Some(new.dir.clone()),
        size: Some(new.size),
        mtime: Some(new.mtime),
        kind: Some(new.kind),
        fingerprint: Some(new.fingerprint.clone()),
    }
}
