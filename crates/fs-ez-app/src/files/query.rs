use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use fs_ez_core::{FileRecord, FileWithTags, FilterExpr, Sort, SortDir, SortField};
use fs_ez_store::{FileListQuery, Page, validate_page_limit};
use time::{Duration, OffsetDateTime};

use super::{FileListMode, FileListRequest};
use crate::app::App;
use crate::error::AppError;

/// Tri stable par identifiant des vues système.
const ID_ASC: Sort = Sort {
    field: SortField::Id,
    dir: SortDir::Asc,
};

impl App {
    pub fn find_by_path(&self, path: &Path) -> Result<Option<FileRecord>, AppError> {
        self.find_many_by_path(&[path.to_path_buf()])
            .map(|mut found| found.pop().flatten())
    }

    /// Résolution groupée équivalente à `find_by_path`, ordre des entrées conservé.
    pub(crate) fn find_many_by_path(
        &self,
        paths: &[PathBuf],
    ) -> Result<Vec<Option<FileRecord>>, AppError> {
        let mut seen = HashSet::with_capacity(paths.len());
        let unique: Vec<PathBuf> = paths
            .iter()
            .filter(|path| seen.insert((*path).clone()))
            .cloned()
            .collect();
        let mut found: HashMap<PathBuf, FileRecord> = HashMap::with_capacity(unique.len());
        self.lookup_paths(&unique, &mut found)?;
        let mut links: HashMap<PathBuf, PathBuf> = HashMap::new();
        let mut candidates = Vec::new();
        for path in &unique {
            if !found.contains_key(path)
                && let Ok(confined) = self.root.confine_existing(path)
            {
                links.insert(path.clone(), confined.clone());
                candidates.push(confined);
            }
        }
        self.lookup_paths(&candidates, &mut found)?;
        let mut candidates = Vec::new();
        for path in &unique {
            if found.contains_key(path)
                || links
                    .get(path)
                    .is_some_and(|candidate| found.contains_key(candidate))
            {
                continue;
            }
            if let Ok(confined) = self.root.confine_new(path) {
                links.insert(path.clone(), confined.clone());
                candidates.push(confined);
            }
        }
        self.lookup_paths(&candidates, &mut found)?;
        Ok(paths
            .iter()
            .map(|path| {
                found
                    .get(path)
                    .cloned()
                    .or_else(|| links.get(path).and_then(|c| found.get(c).cloned()))
            })
            .collect())
    }

    fn lookup_paths(
        &self,
        candidates: &[PathBuf],
        found: &mut HashMap<PathBuf, FileRecord>,
    ) -> Result<(), AppError> {
        let missing: Vec<PathBuf> = candidates
            .iter()
            .filter(|path| !found.contains_key(*path))
            .cloned()
            .collect();
        if missing.is_empty() {
            return Ok(());
        }
        for record in self.store.find_files_by_paths(&missing)? {
            found.entry(record.path.clone()).or_insert(record);
        }
        Ok(())
    }

    pub fn find_by_name(&self, name: &str) -> Result<Option<FileRecord>, AppError> {
        Ok(self.store.find_file_by_name(name)?)
    }

    pub fn list_files(&self, query: &FileListQuery) -> Result<Page<FileWithTags>, AppError> {
        validate_page_limit(query.limit)?;
        Ok(self.store.list_files(query)?)
    }

    /// Une page de fichiers portant tous les `tags`, tri stable par identifiant.
    pub fn files_for_tags(
        &self,
        tags: &[String],
        offset: usize,
        limit: usize,
    ) -> Result<Page<FileWithTags>, AppError> {
        let filter = FilterExpr::all_tags(tags)
            .ok_or_else(|| AppError::Invalid("at least one tag is required".to_string()))?;
        // Le garde d'offset passe avant la validation de la limite.
        Ok(self
            .list_files_page(&FileListRequest {
                filter: Some(filter),
                sort: Some(ID_ASC),
                offset,
                limit,
                ..FileListRequest::new(limit)
            })?
            .into_page())
    }

    /// Page d'une vue système, tri et fenêtres issus des réglages.
    pub fn files_for_mode(
        &self,
        mode: FileListMode,
        offset: usize,
        limit: usize,
    ) -> Result<Page<FileWithTags>, AppError> {
        self.page(&FileListRequest {
            mode,
            offset,
            limit,
            ..FileListRequest::new(limit)
        })
    }

    fn page(&self, request: &FileListRequest) -> Result<Page<FileWithTags>, AppError> {
        validate_page_limit(request.limit)?;
        Ok(self.list_files_page(request)?.into_page())
    }

    /// Appartenance ponctuelle à une vue système, sans parcourir la liste.
    pub fn file_matches_mode(
        &self,
        record: &FileRecord,
        mode: FileListMode,
    ) -> Result<bool, AppError> {
        match mode {
            FileListMode::All => Ok(true),
            FileListMode::Untagged => Ok(self.store.tags_for_file(record.id)?.is_empty()),
            FileListMode::Recent => {
                let cutoff = self.cutoff(self.settings()?.recent_days);
                Ok(self
                    .store
                    .tags_for_file(record.id)?
                    .iter()
                    .any(|applied| applied.created_at >= cutoff))
            }
            FileListMode::RecentFiles => {
                Ok(record.mtime >= self.cutoff(self.settings()?.recent_file_days))
            }
        }
    }

    fn cutoff(&self, days: i64) -> OffsetDateTime {
        self.now()
            .checked_sub(Duration::days(days))
            .unwrap_or(OffsetDateTime::UNIX_EPOCH)
    }

    pub fn recent_files(&self, days: Option<i64>) -> Result<Page<FileWithTags>, AppError> {
        let limit = self.settings()?.page_size;
        self.page(&FileListRequest {
            mode: FileListMode::Recent,
            recent_days: days,
            limit,
            ..FileListRequest::new(limit)
        })
    }

    pub fn untagged_files(&self, limit: usize) -> Result<Page<FileWithTags>, AppError> {
        self.page(&FileListRequest {
            mode: FileListMode::Untagged,
            sort: Some(ID_ASC),
            limit,
            ..FileListRequest::new(limit)
        })
    }
}
