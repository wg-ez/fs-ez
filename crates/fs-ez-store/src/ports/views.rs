//! Port de persistance des vues sauvegardées.

use fs_ez_core::{NewSavedView, SavedView, SavedViewPatch, ViewId};

use crate::error::StoreResult;

pub trait ViewStore {
    fn list_views(&self) -> StoreResult<Vec<SavedView>>;
    fn get_view(&self, id: ViewId) -> StoreResult<Option<SavedView>>;
    fn get_view_by_name(&self, name: &str) -> StoreResult<Option<SavedView>>;
    fn create_view(&self, view: &NewSavedView) -> StoreResult<SavedView>;
    fn update_view(&self, id: ViewId, patch: &SavedViewPatch) -> StoreResult<SavedView>;
    fn delete_view(&self, id: ViewId) -> StoreResult<bool>;
}
