//! Cas d'usage des tags : CRUD et fusion.

use fs_ez_core::{FilterExpr, GroupId, Tag, TagColor, TagCount, TagId};
use fs_ez_store::StoreError;
use rand::TryRng;
use rand::rngs::SysRng;

use crate::app::App;
use crate::error::AppError;
use crate::tagspec::parse_tag_name;

/// Couleur de palette tirée au hasard quand la création n'en impose aucune.
pub fn random_tag_color() -> Result<TagColor, AppError> {
    let mut byte = [0u8; 1];
    let mut rng = SysRng;
    rng.try_fill_bytes(&mut byte)
        .map_err(|error| AppError::Invalid(format!("random source failure: {error}")))?;
    Ok(TagColor::ALL[usize::from(byte[0]) % TagColor::ALL.len()])
}

impl App {
    pub fn list_tags(&self) -> Result<Vec<TagCount>, AppError> {
        Ok(self.store.tag_counts()?)
    }

    /// Tags portés par au moins un fichier de la sélection.
    pub fn tags_for_selection(&self, tags: &[String]) -> Result<Vec<Tag>, AppError> {
        let filter = FilterExpr::all_tags(tags)
            .ok_or_else(|| AppError::Invalid("at least one tag is required".to_string()))?;
        Ok(self.store.tags_for_filter(&filter)?)
    }

    pub fn create_tag(&self, group: GroupId, name: &str, color: TagColor) -> Result<Tag, AppError> {
        self.require_group(group)?;
        Ok(self.store.get_or_create_tag(group, name, color)?)
    }

    pub fn rename_tag(&self, old: &str, new: &str) -> Result<Tag, AppError> {
        let tag = self.require_tag(old)?;
        Ok(self.store.rename_tag(tag.id, new)?)
    }

    pub fn set_tag_color(
        &self,
        name: &str,
        color: TagColor,
        custom: Option<&str>,
    ) -> Result<Tag, AppError> {
        let tag = self.require_tag(name)?;
        Ok(self
            .store
            .set_tag_color(tag.id, color, custom.map(str::to_string))?)
    }

    pub fn delete_tag(&self, name: &str) -> Result<bool, AppError> {
        let tag = self.require_tag(name)?;
        Ok(self.store.delete_tag(tag.id)?)
    }

    pub fn merge_tags(&self, sources: &[String], target: &str) -> Result<u64, AppError> {
        let target = self.require_tag(target)?;
        let mut resolved = Vec::new();
        for source in sources {
            let tag = self.require_tag(source)?;
            if tag.id == target.id {
                return Err(AppError::Invalid(
                    "cannot merge a tag into itself".to_string(),
                ));
            }
            if !resolved.contains(&tag.id) {
                resolved.push(tag.id);
            }
        }
        let mut touched = 0u64;
        for source in resolved {
            match self.store.merge_tags(source, target.id) {
                Ok(count) => touched = touched.saturating_add(count),
                Err(StoreError::NotFound(_)) => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(touched)
    }

    pub fn resolve_tag_name(&self, raw: &str) -> Result<TagId, AppError> {
        let display = parse_tag_name(raw)?;
        self.store
            .get_tag_by_name(&display)?
            .map(|tag| tag.id)
            .ok_or_else(|| AppError::NotFound(format!("tag {display}")))
    }

    pub(crate) fn require_tag(&self, name: &str) -> Result<Tag, AppError> {
        let (display, _) = fs_ez_core::normalize_tag_name(name)?;
        self.store
            .get_tag_by_name(&display)?
            .ok_or_else(|| AppError::NotFound(format!("tag {display}")))
    }
}
