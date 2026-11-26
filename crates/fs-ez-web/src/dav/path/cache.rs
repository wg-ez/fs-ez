use std::ffi::OsString;
use std::time::Duration;

use dav_server::fs::FsError;
use fs_ez_app::App;
use fs_ez_core::{FilterExpr, Tag};

use super::children::tag_lists;
use super::error::fs_error;
use super::model::tag_names;
use crate::dav::cache::TtlMap;

/// TTL court : borne les écritures Web et les DELETE DAV sans vider à chaque fichier.
const SELECTION_TTL: Duration = Duration::from_secs(5);
/// Borne mémoire : une entrée par sélection de tags active.
const SELECTION_CACHE_MAX: usize = 256;
const TAG_LISTS_KEY: &str = "tag_lists";

/// Cache par sélection de tags des deux requêtes SQL lourdes de `resolve`.
#[derive(Clone, Debug)]
pub struct SelectionCache {
    entries: TtlMap<SelectionEntry>,
    tags: TtlMap<TagLists>,
}

impl Default for SelectionCache {
    fn default() -> Self {
        Self {
            entries: TtlMap::new(SELECTION_TTL, SELECTION_CACHE_MAX),
            tags: TtlMap::new(SELECTION_TTL, 1),
        }
    }
}

pub type TagChildren = Vec<(OsString, Tag)>;

/// Tags visibles : portés par au moins un fichier, ou vides dans `All Empty`.
#[derive(Clone, Debug, Default)]
pub struct TagLists {
    pub used: TagChildren,
    pub empty: TagChildren,
}

#[derive(Clone, Copy, Debug, Default)]
struct SelectionEntry {
    complete: Option<bool>,
    exists: Option<bool>,
}

impl SelectionCache {
    pub fn clear(&self) {
        self.entries.clear();
        self.tags.clear();
    }

    /// Enfants de `tags` mis en cache, les collisions de noms sont déjà résolues.
    #[cfg(test)]
    pub(super) fn tag_children(&self, app: &App) -> Result<TagChildren, FsError> {
        Ok(self.tag_lists(app)?.used)
    }

    pub(super) fn tag_lists(&self, app: &App) -> Result<TagLists, FsError> {
        if let Some(lists) = self.tags.get(TAG_LISTS_KEY) {
            return Ok(lists);
        }
        let lists = tag_lists(app).map_err(fs_error)?;
        self.tags.insert(TAG_LISTS_KEY.to_string(), lists.clone());
        Ok(lists)
    }

    pub(super) fn complete(&self, app: &App, tags: &[Tag]) -> Result<bool, FsError> {
        let key = selection_key(tags);
        if let Some(value) = self.entries.get(&key).and_then(|entry| entry.complete) {
            return Ok(value);
        }
        let matching = app.tags_for_selection(&tag_names(tags)).map_err(fs_error)?;
        let complete = matching
            .iter()
            .all(|tag| tags.iter().any(|selected| selected.id == tag.id));
        self.store(&key, |entry| entry.complete = Some(complete));
        Ok(complete)
    }

    pub(super) fn exists(&self, app: &App, tags: &[Tag]) -> Result<bool, FsError> {
        let key = selection_key(tags);
        if let Some(value) = self.entries.get(&key).and_then(|entry| entry.exists) {
            return Ok(value);
        }
        let filter = FilterExpr::all_tags(&tag_names(tags)).ok_or(FsError::NotFound)?;
        let exists = app.count_filter(&filter).map_err(fs_error)? != 0;
        self.store(&key, |entry| entry.exists = Some(exists));
        Ok(exists)
    }

    fn store(&self, key: &str, update: impl FnOnce(&mut SelectionEntry)) {
        let mut entry = self.entries.get(key).unwrap_or_default();
        update(&mut entry);
        self.entries.insert(key.to_string(), entry);
    }
}

fn selection_key(tags: &[Tag]) -> String {
    tags.iter()
        .map(|tag| tag.id.0.to_string())
        .collect::<Vec<_>>()
        .join(",")
}
