//! Port de persistance des groupes de vues.

use fs_ez_core::{SavedView, TagColor, ViewGroup, ViewGroupId, ViewId};

use crate::error::StoreResult;

pub trait ViewGroupStore {
    fn list_view_groups(&self) -> StoreResult<Vec<ViewGroup>>;
    fn get_view_group(&self, id: ViewGroupId) -> StoreResult<Option<ViewGroup>>;
    fn create_view_group(&self, name: &str, color: Option<TagColor>) -> StoreResult<ViewGroup>;
    fn update_view_group(
        &self,
        id: ViewGroupId,
        name: Option<&str>,
        color: Option<Option<TagColor>>,
        position: Option<i64>,
        collapsed: Option<bool>,
    ) -> StoreResult<ViewGroup>;
    /// Supprime le groupe et toutes ses vues.
    fn delete_view_group(&self, id: ViewGroupId) -> StoreResult<bool>;
    /// Groupe unique de la vue, `None` si la vue n'appartient à aucun groupe.
    fn group_for_view(&self, view: ViewId) -> StoreResult<Option<ViewGroupId>>;
    /// Place la vue dans `group`, avant `before` ou en fin de groupe.
    fn place_view(
        &self,
        group: ViewGroupId,
        view: ViewId,
        before: Option<ViewId>,
    ) -> StoreResult<()>;
    /// Applique l'ordre donné aux groupes, index = nouvelle position.
    fn reorder_view_groups(&self, ids: &[ViewGroupId]) -> StoreResult<()>;
    /// Applique l'ordre donné aux vues d'un groupe, index = nouvelle position.
    fn reorder_group_views(&self, group: ViewGroupId, ids: &[ViewId]) -> StoreResult<()>;
    fn views_in_group(&self, group: ViewGroupId) -> StoreResult<Vec<SavedView>>;
}
