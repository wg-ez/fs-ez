//! Port de persistance des groupes de tags.

use fs_ez_core::{GroupId, Tag, TagColor, TagGroup, TagId};

use crate::error::StoreResult;

pub trait GroupStore {
    fn list_groups(&self) -> StoreResult<Vec<TagGroup>>;
    fn get_group(&self, id: GroupId) -> StoreResult<Option<TagGroup>>;
    fn create_group(&self, name: &str, color: Option<TagColor>) -> StoreResult<TagGroup>;
    fn update_group(
        &self,
        id: GroupId,
        name: Option<&str>,
        color: Option<Option<TagColor>>,
        position: Option<i64>,
        collapsed: Option<bool>,
    ) -> StoreResult<TagGroup>;
    /// Supprime le groupe et tous ses tags.
    fn delete_group(&self, id: GroupId) -> StoreResult<bool>;
    /// Groupe unique du tag, `None` si le tag n'appartient à aucun groupe.
    fn group_for_tag(&self, tag: TagId) -> StoreResult<Option<GroupId>>;
    /// Place le tag dans `group`, avant `before` ou en fin de groupe.
    fn place_tag(&self, group: GroupId, tag: TagId, before: Option<TagId>) -> StoreResult<()>;
    /// Applique l'ordre donné aux groupes, index = nouvelle position.
    fn reorder_groups(&self, ids: &[GroupId]) -> StoreResult<()>;
    /// Applique l'ordre donné aux tags d'un groupe, index = nouvelle position.
    fn reorder_group_tags(&self, group: GroupId, ids: &[TagId]) -> StoreResult<()>;
    fn tags_in_group(&self, group: GroupId) -> StoreResult<Vec<Tag>>;
}
