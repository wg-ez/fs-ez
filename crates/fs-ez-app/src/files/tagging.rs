use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use fs_ez_core::{FileId, FileKind, FileWithTags, TagId};

use super::EnsureOptions;
use crate::app::App;
use crate::error::AppError;
use crate::tags::random_tag_color;
use crate::tagspec::parse_tag_names;

impl App {
    pub fn tag_path(
        &self,
        path: &Path,
        tags: &[String],
        force: bool,
    ) -> Result<FileWithTags, AppError> {
        let file = self.ensure_file(path, EnsureOptions { force })?;
        let tags = parse_tag_names(tags)?;
        self.apply_tags(file.id, &tags, true)?;
        self.file_with_tags(file.id)
    }

    /// Retire uniquement les tags demandés.
    pub fn untag_path(&self, path: &Path, tags: &[String]) -> Result<FileWithTags, AppError> {
        // Une entrée peut rester en base après suppression disque, `untag` doit encore fonctionner.
        let file = match self.find_by_path(path)? {
            Some(file) => file,
            None => self.ensure_file(path, EnsureOptions { force: false })?,
        };
        let tags = parse_tag_names(tags)?;
        self.remove_tags(file.id, &tags)?;
        self.file_with_tags(file.id)
    }

    /// Application groupée : une résolution de chemins et un ajout SQL pour tout le lot.
    pub fn tag_paths(
        &self,
        paths: &[PathBuf],
        tags: &[String],
        force: bool,
    ) -> Result<Vec<FileWithTags>, AppError> {
        let tags = parse_tag_names(tags)?;
        let ids = self.resolve_path_ids(paths, Some(force))?;
        self.apply_tags_bulk(&ids, &tags, true)?;
        self.files_with_tags(&ids)
    }

    /// Retrait groupé : `untag_path` ne rafraîchit jamais un enregistrement connu.
    pub fn untag_paths(
        &self,
        paths: &[PathBuf],
        tags: &[String],
    ) -> Result<Vec<FileWithTags>, AppError> {
        let tags = parse_tag_names(tags)?;
        let ids = self.resolve_path_ids(paths, None)?;
        self.remove_tags_bulk(&ids, &tags)?;
        self.files_with_tags(&ids)
    }

    /// Résolution path → id dans l'ordre d'entrée, `Some(force)` rafraîchit, `None` réutilise.
    fn resolve_path_ids(
        &self,
        paths: &[PathBuf],
        force: Option<bool>,
    ) -> Result<Vec<FileId>, AppError> {
        let known = self.find_many_by_path(paths)?;
        let mut ids = Vec::with_capacity(paths.len());
        for (path, record) in paths.iter().zip(known) {
            let file = match record {
                // Métadonnées inchangées : ni relecture du contenu ni UPDATE inutile.
                Some(record) if force.is_none() || self.record_is_fresh(path, &record) => record,
                _ => self.ensure_file(
                    path,
                    EnsureOptions {
                        force: force.unwrap_or(false),
                    },
                )?,
            };
            ids.push(file.id);
        }
        Ok(ids)
    }

    /// `false` dès qu'un stat échoue, diffère ou qu'une empreinte manque.
    fn record_is_fresh(&self, path: &Path, record: &fs_ez_core::FileRecord) -> bool {
        match self.fs.metadata(path) {
            Ok(meta) => {
                !meta.is_special
                    && meta.kind == record.kind
                    && meta.size == record.size
                    && meta.mtime == record.mtime
                    && (meta.kind != FileKind::File || record.fingerprint.is_some())
            }
            Err(_) => false,
        }
    }

    /// Résultat par identifiant, doublons d'entrée partagés, ordre conservé.
    fn files_with_tags(&self, ids: &[FileId]) -> Result<Vec<FileWithTags>, AppError> {
        let mut cache: HashMap<FileId, FileWithTags> = HashMap::new();
        for id in ids {
            if !cache.contains_key(id) {
                cache.insert(*id, self.file_with_tags(*id)?);
            }
        }
        ids.iter()
            .map(|id| {
                cache
                    .get(id)
                    .cloned()
                    .ok_or_else(|| AppError::NotFound(format!("file {}", id.0)))
            })
            .collect()
    }

    pub fn apply_tags(
        &self,
        file: FileId,
        tags: &[String],
        explicit: bool,
    ) -> Result<(), AppError> {
        let seeds = self.resolve_or_create(tags)?;
        let at = self.now();
        for tag in seeds {
            self.store.apply_tag(file, tag, explicit, at)?;
        }
        Ok(())
    }

    pub fn remove_tags(&self, file: FileId, tags: &[String]) -> Result<(), AppError> {
        for tag in tags {
            if let Some(tag) = self.store.get_tag_by_name(tag)? {
                self.store.remove_tag(file, tag.id)?;
            }
        }
        Ok(())
    }

    /// Remplace tous les tags du fichier.
    pub fn set_tags(&self, file: FileId, tags: &[String]) -> Result<(), AppError> {
        for applied in self.store.tags_for_file(file)? {
            self.store.remove_tag(file, applied.tag.id)?;
        }
        self.apply_tags(file, tags, true)
    }

    /// Ajout groupé : tags résolus une fois, même sémantique que `apply_tags` par fichier.
    pub fn apply_tags_bulk(
        &self,
        files: &[FileId],
        tags: &[String],
        explicit: bool,
    ) -> Result<usize, AppError> {
        if files.is_empty() {
            return Ok(0);
        }
        let seeds = self.resolve_or_create(tags)?;
        if seeds.is_empty() {
            return Ok(files.len());
        }
        Ok(self
            .store
            .add_tags_batch(files, &seeds, explicit, self.now())?)
    }

    /// Retrait groupé : tags absents ignorés, même sémantique que `remove_tags`.
    pub fn remove_tags_bulk(&self, files: &[FileId], tags: &[String]) -> Result<usize, AppError> {
        if files.is_empty() {
            return Ok(0);
        }
        let mut seeds = Vec::new();
        let mut seen = HashSet::new();
        for name in tags {
            if let Some(tag) = self.store.get_tag_by_name(name)?
                && seen.insert(tag.id)
            {
                seeds.push(tag.id);
            }
        }
        if seeds.is_empty() {
            return Ok(files.len());
        }
        Ok(self.store.remove_tags_batch(files, &seeds)?)
    }

    /// Remplacement groupé : tags résolus une fois, purge puis ajout explicite.
    pub fn set_tags_bulk(&self, files: &[FileId], tags: &[String]) -> Result<usize, AppError> {
        if files.is_empty() {
            return Ok(0);
        }
        let seeds = self.resolve_or_create(tags)?;
        Ok(self.store.replace_tags_batch(files, &seeds, self.now())?)
    }

    pub fn file_with_tags(&self, id: FileId) -> Result<FileWithTags, AppError> {
        let file = self
            .store
            .get_file(id)?
            .ok_or_else(|| AppError::NotFound(format!("file {}", id.0)))?;
        let tags = self.store.tags_for_file(id)?;
        Ok(FileWithTags { file, tags })
    }

    fn resolve_or_create(&self, tags: &[String]) -> Result<Vec<TagId>, AppError> {
        if tags.len() > crate::tagspec::MAX_TAGS_PER_REQUEST {
            return Err(AppError::Invalid(format!(
                "too many tags (max {})",
                crate::tagspec::MAX_TAGS_PER_REQUEST
            )));
        }
        let mut seeds = Vec::new();
        let mut seen = HashSet::new();
        let mut fallback = None;
        for name in tags {
            let tag = match self.store.get_tag_by_name(name)? {
                Some(tag) => tag,
                None => {
                    let group = match fallback {
                        Some(group) => group,
                        None => {
                            let group = self.fallback_group()?;
                            fallback = Some(group);
                            group
                        }
                    };
                    self.create_tag(group, name, random_tag_color()?)?
                }
            };
            if seen.insert(tag.id) {
                seeds.push(tag.id);
            }
        }
        Ok(seeds)
    }
}
