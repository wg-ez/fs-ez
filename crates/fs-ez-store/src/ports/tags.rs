//! Port de persistance des tags.

use fs_ez_core::{FilterExpr, GroupId, Tag, TagColor, TagCount, TagId};

use crate::error::StoreResult;

pub trait TagStore {
    /// Crée le tag dans `group`, ou retourne le tag existant sans changer son groupe.
    fn get_or_create_tag(&self, group: GroupId, name: &str, color: TagColor) -> StoreResult<Tag>;
    fn get_tag(&self, id: TagId) -> StoreResult<Option<Tag>>;
    fn get_tag_by_name(&self, name: &str) -> StoreResult<Option<Tag>>;
    fn list_tags(&self) -> StoreResult<Vec<Tag>>;
    fn rename_tag(&self, id: TagId, name: &str) -> StoreResult<Tag>;
    fn set_tag_color(&self, id: TagId, color: TagColor, custom: Option<String>)
    -> StoreResult<Tag>;
    fn delete_tag(&self, id: TagId) -> StoreResult<bool>;
    /// Retourne le nombre de fichiers touchés.
    fn merge_tags(&self, source: TagId, target: TagId) -> StoreResult<u64>;

    fn tag_counts(&self) -> StoreResult<Vec<TagCount>>;
    /// Tags distincts portés par au moins un fichier du filtre.
    fn tags_for_filter(&self, filter: &FilterExpr) -> StoreResult<Vec<Tag>>;
}
