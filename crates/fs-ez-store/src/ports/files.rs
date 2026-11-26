//! Port de persistance des fichiers et de leurs tags.

use std::path::{Path, PathBuf};

use fs_ez_core::{
    AppliedTag, FileId, FilePatch, FileRecord, FileWithTags, FilterExpr, Fingerprint, NewFile,
    TagId,
};
use time::OffsetDateTime;

use crate::error::StoreResult;
use crate::page::Page;
use crate::types::FileListQuery;

/// Collecte les éléments trouvés dans l'ordre de `items`, les absents sont ignorés.
fn collect_found<T, U>(
    items: &[T],
    find: impl Fn(&T) -> StoreResult<Option<U>>,
) -> StoreResult<Vec<U>> {
    let mut found = Vec::with_capacity(items.len());
    for item in items {
        if let Some(value) = find(item)? {
            found.push(value);
        }
    }
    Ok(found)
}

pub trait FileStore {
    fn insert_file(&self, new: &NewFile) -> StoreResult<FileRecord>;
    fn update_file(&self, id: FileId, patch: &FilePatch) -> StoreResult<FileRecord>;
    /// Compare-and-set du chemin : `true` seulement si la ligne portait encore `expected_path`.
    fn update_file_path_if(
        &self,
        id: FileId,
        expected_path: &Path,
        patch: &FilePatch,
    ) -> StoreResult<bool> {
        match self.get_file(id)? {
            Some(current) if current.path == expected_path => {
                self.update_file(id, patch)?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }
    /// Mise à jour groupée transactionnelle, tout ou rien, boucle par défaut.
    fn update_files_batch(&self, updates: &[(FileId, FilePatch)]) -> StoreResult<usize> {
        for (id, patch) in updates {
            self.update_file(*id, patch)?;
        }
        Ok(updates.len())
    }
    fn get_file(&self, id: FileId) -> StoreResult<Option<FileRecord>>;
    /// Variante par lot, une requête groupée côté SQL, boucle par défaut.
    fn get_files_by_ids(&self, ids: &[FileId]) -> StoreResult<Vec<FileRecord>> {
        collect_found(ids, |id| self.get_file(*id))
    }
    fn find_file_by_path(&self, path: &Path) -> StoreResult<Option<FileRecord>>;
    /// Première ligne par identifiant pour un nom logique, le namespace DAV est plat.
    fn find_file_by_name(&self, name: &str) -> StoreResult<Option<FileRecord>>;
    fn find_file_by_fingerprint(&self, fp: &Fingerprint) -> StoreResult<Option<FileRecord>>;
    /// Variante par lot, une requête groupée côté SQL, boucle par défaut.
    fn find_files_by_fingerprints(&self, fps: &[Fingerprint]) -> StoreResult<Vec<FileRecord>> {
        collect_found(fps, |fp| self.find_file_by_fingerprint(fp))
    }
    /// Variante par lot des destinations, une requête groupée côté SQL.
    fn find_files_by_paths(&self, paths: &[PathBuf]) -> StoreResult<Vec<FileRecord>> {
        collect_found(paths, |path| self.find_file_by_path(path))
    }
    /// Insertion transactionnelle, tout ou rien, boucle par défaut.
    fn insert_files_batch(&self, files: &[NewFile]) -> StoreResult<Vec<FileId>> {
        let mut ids = Vec::with_capacity(files.len());
        for file in files {
            ids.push(self.insert_file(file)?.id);
        }
        Ok(ids)
    }
    fn delete_file(&self, id: FileId) -> StoreResult<bool>;
    /// Suppression groupée transactionnelle, boucle unitaire par défaut.
    fn delete_files_batch(&self, ids: &[FileId]) -> StoreResult<usize> {
        let mut deleted = 0;
        for id in ids {
            if self.delete_file(*id)? {
                deleted += 1;
            }
        }
        Ok(deleted)
    }
    fn list_files(&self, query: &FileListQuery) -> StoreResult<Page<FileWithTags>>;
    /// Nombre de fichiers de la requête, offset et limite ignorés.
    fn count_files_query(&self, query: &FileListQuery) -> StoreResult<u64>;
    fn tags_for_file(&self, id: FileId) -> StoreResult<Vec<AppliedTag>>;
    /// `true` si insertion ou promotion de `explicit = 0` vers `explicit = 1`.
    fn apply_tag(
        &self,
        file: FileId,
        tag: TagId,
        explicit: bool,
        at: OffsetDateTime,
    ) -> StoreResult<bool>;
    fn remove_tag(&self, file: FileId, tag: TagId) -> StoreResult<bool>;
    /// Ajout groupé, nombre d'associations créées ou promues, boucle par défaut.
    fn add_tags_batch(
        &self,
        files: &[FileId],
        tags: &[TagId],
        explicit: bool,
        at: OffsetDateTime,
    ) -> StoreResult<usize> {
        let mut changed = 0;
        for file in files {
            for tag in tags {
                if self.apply_tag(*file, *tag, explicit, at)? {
                    changed += 1;
                }
            }
        }
        Ok(changed)
    }
    /// Retrait groupé, nombre d'associations supprimées, boucle par défaut.
    fn remove_tags_batch(&self, files: &[FileId], tags: &[TagId]) -> StoreResult<usize> {
        let mut removed = 0;
        for file in files {
            for tag in tags {
                if self.remove_tag(*file, *tag)? {
                    removed += 1;
                }
            }
        }
        Ok(removed)
    }
    /// Remplacement groupé du jeu de tags, nombre d'associations créées, boucle par défaut.
    fn replace_tags_batch(
        &self,
        files: &[FileId],
        tags: &[TagId],
        at: OffsetDateTime,
    ) -> StoreResult<usize> {
        let mut applied = 0;
        for file in files {
            for old in self.tags_for_file(*file)? {
                self.remove_tag(*file, old.tag.id)?;
            }
            for tag in tags {
                if self.apply_tag(*file, *tag, true, at)? {
                    applied += 1;
                }
            }
        }
        Ok(applied)
    }
    fn all_files(&self) -> StoreResult<Vec<FileRecord>>;
    fn count_files(&self) -> StoreResult<u64>;
    fn count_untagged(&self) -> StoreResult<u64>;
    fn count_tagged_since(&self, days: i64) -> StoreResult<u64>;
    fn count_matching(&self, filter: &FilterExpr) -> StoreResult<u64>;
    /// Vrai si le fichier satisfait le filtre, sans charger la liste.
    fn file_matches_filter(&self, id: FileId, filter: &FilterExpr) -> StoreResult<bool>;
    /// `PRAGMA data_version` pour détecter les écritures externes (fiche 42).
    fn data_version(&self) -> StoreResult<i64>;
}
