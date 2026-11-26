//! Lecture et remplacement atomique du contenu d'un fichier référencé.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use fs_ez_core::{FileId, FileKind, FilePatch, FileRecord};
use fs_ez_fs::{FsError, display_lossy};
use time::OffsetDateTime;

use crate::app::App;
use crate::error::{AppError, io_error};

const TEMP_MODE: u32 = 0o600;

#[derive(Debug, Clone)]
pub struct ReadTarget {
    pub record: FileRecord,
    pub path: PathBuf,
}

impl App {
    /// Chemin réel confiné sous la racine, jamais dans le dossier d'état, uniquement pour un fichier régulier.
    pub fn read_target(&self, id: FileId) -> Result<ReadTarget, AppError> {
        let record = self
            .store
            .get_file(id)?
            .ok_or_else(|| AppError::NotFound(format!("file {}", id.0)))?;
        self.read_target_resolved(record)
    }

    /// Variante DAV : enregistrement déjà résolu, aucune relecture `get_file`.
    pub fn read_target_resolved(&self, record: FileRecord) -> Result<ReadTarget, AppError> {
        ensure_regular_file(&record)?;
        let path = self.confine_content_path(&record.path)?;
        let meta = match fs::metadata(&path) {
            Ok(meta) => meta,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(AppError::NotFound(display_lossy(&path)));
            }
            Err(error) => return Err(io_error(&path, error)),
        };
        if !meta.is_file() {
            return Err(AppError::Invalid(format!(
                "not a regular file: {}",
                display_lossy(&path)
            )));
        }
        Ok(ReadTarget { record, path })
    }

    /// Copie `source` dans un temporaire du dossier cible, fsync, permissions et rename atomique.
    pub fn replace_file_contents(
        &self,
        id: FileId,
        source: &Path,
        mtime: Option<OffsetDateTime>,
    ) -> Result<FileRecord, AppError> {
        // Verrou tenu jusqu'au bout : un rename du même id ne peut pas déplacer la cible.
        let _mutation = self.lock_file(id);
        let record = self
            .store
            .get_file(id)?
            .ok_or_else(|| AppError::NotFound(format!("file {}", id.0)))?;
        ensure_regular_file(&record)?;
        let source = fs::canonicalize(source).map_err(|error| io_error(source, error))?;
        // État séparé de la racine : le staging DAV reste sous `upload_tmp_dir`.
        if !source.starts_with(self.root.state_dir()) && !source.starts_with(self.upload_tmp_dir())
        {
            return Err(AppError::Invalid(
                "source is outside the temporary directories".to_string(),
            ));
        }
        let target = self.confine_content_path(&record.path)?;
        let parent = target
            .parent()
            .ok_or_else(|| AppError::Invalid("target has no parent".to_string()))?;
        let previous = fs::metadata(&target).ok();
        let temp = parent.join(crate::staging::content_part(&crate::token::random_token()?));

        let result = self.write_replacement(&source, &temp, &target, previous.as_ref());
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result?;
        let _ = fs::remove_file(&source);

        if let Some(mtime) = mtime
            && let Err(error) = self.fs.set_mtime(&target, mtime)
        {
            tracing::warn!(path = %display_lossy(&target), %error, "replace mtime restore failed");
        }
        let meta = fs::metadata(&target).map_err(|error| io_error(&target, error))?;
        let mtime = meta.modified().map_err(|error| io_error(&target, error))?;
        let patch = FilePatch {
            path: None,
            path_display: None,
            name: None,
            dir: None,
            size: Some(meta.len()),
            mtime: Some(OffsetDateTime::from(mtime)),
            kind: None,
            // Empreinte effacée : le prochain scan la recalcule.
            fingerprint: Some(None),
        };
        match self.store.update_file(id, &patch) {
            Ok(record) => Ok(record),
            Err(error) => {
                // Le fichier est déjà écrit : une erreur de métadonnées ne doit pas faire échouer PUT/MOVE.
                tracing::warn!(file = id.0, %error, "métadonnées non mises à jour après écriture");
                Ok(record)
            }
        }
    }

    fn write_replacement(
        &self,
        source: &Path,
        temp: &Path,
        target: &Path,
        previous: Option<&fs::Metadata>,
    ) -> Result<(), AppError> {
        let mut input = File::open(source).map_err(|error| io_error(source, error))?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(TEMP_MODE)
            .open(temp)
            .map_err(|error| io_error(temp, error))?;
        io::copy(&mut input, &mut output).map_err(|error| io_error(temp, error))?;
        output.sync_all().map_err(|error| io_error(temp, error))?;
        drop(output);
        if let Some(previous) = previous {
            let _ = fs::set_permissions(temp, previous.permissions());
            let _ = std::os::unix::fs::chown(temp, Some(previous.uid()), Some(previous.gid()));
        }
        fs::rename(temp, target).map_err(|error| io_error(target, error))?;
        if let Some(parent) = target.parent()
            && let Ok(dir) = File::open(parent)
        {
            let _ = dir.sync_all();
        }
        Ok(())
    }

    fn confine_content_path(&self, path: &Path) -> Result<PathBuf, AppError> {
        let confined = match self.root.confine_existing(path) {
            Ok(path) => path,
            Err(FsError::NotFound(_)) => self.root.confine_new(path)?,
            Err(error) => return Err(error.into()),
        };
        self.ensure_outside_state(&confined)?;
        Ok(confined)
    }
}

fn ensure_regular_file(record: &FileRecord) -> Result<(), AppError> {
    if record.kind == FileKind::File {
        return Ok(());
    }
    Err(AppError::Invalid(format!(
        "not a regular file: {}",
        record.path_display
    )))
}
