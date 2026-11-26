//! Indexation en bibliothèque d'un chemin ou d'un upload déjà placé.

use std::ffi::OsStr;
use std::path::Path;

use fs_ez_core::{FileKind, Fingerprint};

use super::dedup::FingerprintResolution;
use super::naming::is_suffixed_copy;
use super::*;
use crate::files::sharded_path;

impl App {
    pub fn import_path(&self, path: &Path) -> Result<LibraryOutcome, AppError> {
        self.index_path(path)
    }

    pub(super) fn index_path(&self, path: &Path) -> Result<LibraryOutcome, AppError> {
        self.index_path_with_fingerprint(path, None, None)
    }

    /// Variante avec empreinte déjà calculée : évite un second hachage après un placement.
    /// `requested` n'est fourni que par un upload au nom propre, seul cas autorisé à déloger un suffixe.
    pub(super) fn index_path_with_fingerprint(
        &self,
        path: &Path,
        fingerprint: Option<Fingerprint>,
        requested: Option<&str>,
    ) -> Result<LibraryOutcome, AppError> {
        let confined = self.root.confine_existing(path)?;
        let meta = self.fs.metadata(&confined)?;
        if meta.is_special {
            return Err(AppError::Invalid(format!(
                "special file: {}",
                display_lossy(&confined)
            )));
        }
        if meta.kind != FileKind::File {
            return Err(AppError::Invalid(format!(
                "not a file: {}",
                display_lossy(&confined)
            )));
        }
        let mut fingerprint = match fingerprint {
            Some(fingerprint) => fingerprint,
            None => self.fingerprints.fingerprint(&confined, meta.kind)?,
        };
        // Même contenu sur un autre chemin = doublon, même chemin = contenu modifié.
        match self.resolve_fingerprint(
            &confined,
            fingerprint.clone(),
            self.fingerprint_covers_file(meta.size),
        )? {
            FingerprintResolution::Available(resolved) => fingerprint = resolved,
            FingerprintResolution::Duplicate(existing) => {
                if let Some(updated) = self.replace_suffixed_duplicate(
                    requested,
                    &confined,
                    &meta,
                    &fingerprint,
                    &existing,
                )? {
                    return Ok(LibraryOutcome::Indexed(updated));
                }
                // Repli : doublon ordinaire, `finish_upload` supprimera la copie au nom propre.
                let existing = self
                    .store
                    .find_file_by_fingerprint(&fingerprint)?
                    .unwrap_or(existing);
                return Ok(LibraryOutcome::Duplicate { existing });
            }
        }
        let new = crate::files::new_file_from_meta(&confined, &meta, Some(fingerprint.clone()));
        let file = match self.store.find_file_by_path(&confined)? {
            Some(existing) => {
                let patch = crate::files::patch_from_new(&new);
                self.store.update_file(existing.id, &patch)?
            }
            None => match self.store.insert_file(&new) {
                Ok(file) => file,
                Err(error) => {
                    // Insertion concurrente du même chemin : la ligne décrit déjà ce fichier.
                    if let Some(existing) = self.store.find_file_by_fingerprint(&fingerprint)? {
                        if existing.path == confined {
                            return Ok(LibraryOutcome::Indexed(existing));
                        }
                        return Ok(LibraryOutcome::Duplicate { existing });
                    }
                    return Err(error.into());
                }
            },
        };
        Ok(LibraryOutcome::Indexed(file))
    }

    /// Nom propre face à une copie suffixée gérée : la ligne migre, `id` et tags conservés.
    /// `None` laisse l'appelant traiter le cas comme un doublon ordinaire.
    fn replace_suffixed_duplicate(
        &self,
        requested: Option<&str>,
        clean: &Path,
        meta: &fs_ez_fs::FileMeta,
        fingerprint: &Fingerprint,
        existing: &FileRecord,
    ) -> Result<Option<FileRecord>, AppError> {
        let Some(requested) = requested else {
            return Ok(None);
        };
        if existing.kind != FileKind::File {
            return Ok(None);
        }
        let Some(candidate) = existing.path.file_name().and_then(OsStr::to_str) else {
            return Ok(None);
        };
        if !is_suffixed_copy(requested, candidate) {
            return Ok(None);
        }
        // Le shard se dérive du nom : l'égalité de chemin prouve la copie gérée du même upload.
        let files = self.files_path()?;
        let expected = sharded_path(&files, OsStr::new(candidate));
        let old_path = match self.root.confine_existing(&expected) {
            Ok(path) if path == existing.path => path,
            _ => return Ok(None),
        };
        let new = crate::files::new_file_from_meta(clean, meta, Some(fingerprint.clone()));
        let patch = crate::files::patch_from_new(&new);
        // CAS : jamais écraser une mutation concurrente de la ligne.
        if !self
            .store
            .update_file_path_if(existing.id, &old_path, &patch)?
        {
            tracing::warn!(
                id = %existing.id,
                path = %existing.path_display,
                "suffixed duplicate swap lost a concurrent update"
            );
            return Ok(None);
        }
        // Ancienne copie d'upload gérée : jamais un fichier utilisateur, suppression best-effort.
        self.remove_quiet(&old_path, "suffixed duplicate cleanup failed");
        // Relecture perdue : la copie au nom propre reste en place, un scan la réindexera.
        Ok(Some(match self.store.get_file(existing.id)? {
            Some(record) => record,
            None => existing.clone(),
        }))
    }
}
