use std::ffi::OsStr;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use fs_ez_core::{FileId, FileKind, FilePatch, FileRecord, has_unsafe_text};
use fs_ez_fs::{FsError, dir_lossy, display_lossy};

use crate::app::App;
use crate::error::{AppError, io_error};
use crate::files::sharded_path;

impl App {
    /// Renomme sur disque puis aligne la ligne, rollback best-effort si la base refuse.
    pub fn rename_file(&self, id: FileId, new_name: &str) -> Result<FileRecord, AppError> {
        // Verrou tenu jusqu'au bout : le remplacement de contenu du même id attend.
        let _mutation = self.lock_file(id);
        let record = self
            .store
            .get_file(id)?
            .ok_or_else(|| AppError::NotFound(format!("file {}", id.0)))?;
        self.rename_locked(record, new_name)
    }

    /// Variante DAV : enregistrement déjà résolu, aucune relecture `get_file`.
    pub fn rename_resolved_file(
        &self,
        record: FileRecord,
        new_name: &str,
    ) -> Result<FileRecord, AppError> {
        let _mutation = self.lock_file(record.id);
        self.rename_locked(record, new_name)
    }

    fn rename_locked(&self, record: FileRecord, new_name: &str) -> Result<FileRecord, AppError> {
        let id = record.id;
        if record.kind == FileKind::Directory {
            return Err(AppError::Invalid(format!(
                "not a file: {}",
                record.path_display
            )));
        }
        let name = validate_file_name(new_name)?;
        if name == record.name {
            return Ok(record);
        }
        let source = self.confine_entry_path(&record.path)?;
        let parent = source
            .parent()
            .ok_or_else(|| AppError::Invalid("file has no parent".to_string()))?;
        let files = self.files_path()?;
        let files = self.root.confine_existing(&files).unwrap_or(files);
        let target = if source.starts_with(&files) {
            let target = sharded_path(&files, OsStr::new(name));
            if let Some(shard) = target.parent() {
                self.fs.create_dir_all(shard)?;
            }
            self.confine_entry_path(&target)?
        } else {
            // Fichier importé hors du dossier files : même dossier parent.
            self.confine_entry_path(&parent.join(name))?
        };
        if let Some(existing) = self.store.find_file_by_path(&target)?
            && existing.id != id
        {
            return Err(AppError::Conflict(format!(
                "target already indexed: {}",
                display_lossy(&target)
            )));
        }
        match self.fs.link_noreplace(&source, &target) {
            Ok(()) => {}
            Err(FsError::AlreadyExists(_)) => {
                return Err(AppError::Conflict(format!(
                    "target already exists: {}",
                    display_lossy(&target)
                )));
            }
            Err(error) => return Err(error.into()),
        }
        // Entre le lien et le unlink, la source a pu recevoir un remplacement de contenu.
        match same_inode(&source, &target) {
            Ok(true) => {}
            Ok(false) => {
                self.release_link(id, &target);
                return Err(AppError::Conflict(format!(
                    "source changed during rename: {}",
                    display_lossy(&source)
                )));
            }
            Err(error) => {
                self.release_link(id, &target);
                return Err(error);
            }
        }
        if let Err(error) = self.fs.remove_file(&source) {
            // Seule la revendication de cet appel est retirée, la source reste intacte.
            self.release_link(id, &target);
            return Err(error.into());
        }
        let patch = FilePatch {
            path: Some(target.clone()),
            path_display: Some(display_lossy(&target)),
            name: Some(name.to_string()),
            dir: Some(dir_lossy(&target).unwrap_or_default()),
            size: None,
            mtime: None,
            kind: None,
            fingerprint: None,
        };
        match self.store.update_file(id, &patch) {
            Ok(record) => Ok(record),
            Err(error) => {
                match self.fs.link_noreplace(&target, &source) {
                    Ok(()) => {
                        if let Err(rollback) = self.fs.remove_file(&target) {
                            tracing::error!(file = id.0, %rollback, "rename rollback failed");
                        }
                    }
                    Err(rollback) => {
                        tracing::error!(file = id.0, %rollback, "rename rollback failed")
                    }
                }
                Err(error.into())
            }
        }
    }
}

impl App {
    /// Retire le lien de revendication, jamais un nom déjà remplacé par un autre écrivain.
    fn release_link(&self, id: FileId, target: &Path) {
        if let Err(rollback) = self.fs.remove_file(target) {
            tracing::error!(file = id.0, %rollback, "rename release failed");
        }
    }
}

/// Même inode des deux noms, la comparaison dev+ino détecte un remplacement concurrent.
fn same_inode(left: &Path, right: &Path) -> Result<bool, AppError> {
    let left = std::fs::symlink_metadata(left).map_err(|error| io_error(left, error))?;
    let right = std::fs::symlink_metadata(right).map_err(|error| io_error(right, error))?;
    Ok(left.dev() == right.dev() && left.ino() == right.ino())
}

/// Nom simple uniquement : pas de séparateur, pas de texte à risque, 255 octets max.
fn validate_file_name(raw: &str) -> Result<&str, AppError> {
    let name = raw.trim();
    if name.is_empty() {
        return Err(AppError::Invalid("empty file name".to_string()));
    }
    if name == "." || name == ".." || name.contains('/') || has_unsafe_text(name) {
        return Err(AppError::Invalid(format!("invalid file name: {name:?}")));
    }
    if name.len() > 255 {
        return Err(AppError::Invalid("file name too long".to_string()));
    }
    Ok(name)
}
