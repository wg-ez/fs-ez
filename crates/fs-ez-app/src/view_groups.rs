//! Cas d'usage des groupes de vues : CRUD et placement des vues.

use fs_ez_core::{SavedView, TagColor, ViewGroup, ViewGroupId, ViewId};

use crate::app::App;
use crate::error::AppError;
use crate::groups::group_ops;

group_ops! {
    group = ViewGroup, item = SavedView, id = ViewGroupId, item_id = ViewId,
    list = list_view_groups, items_in = views_in_group,
    with_items = view_groups_with_views, one_with_items = view_group_with_views,
    create = create_view_group, rename = rename_view_group,
    set_color = set_view_group_color, set_collapsed = set_view_group_collapsed,
    delete = delete_view_group,
    assign = assign_view, place = place_view, prepend = prepend_view,
    reorder = reorder_view_groups, reorder_items = reorder_group_views,
    fallback = fallback_view_group, require = require_view_group,
    update = update_view_group, group_for = group_for_view, get = get_view_group,
    item_arg = view: ViewId, resolve = get_view,
    delete_doc = "Supprime le groupe et toutes ses vues.",
    place_doc = "Place une vue dans un groupe, avant `before` ou en fin de groupe.",
    prepend_doc = "Nouvelle vue en tête du groupe, sans déplacer les autres membres.",
    fallback_doc = "Groupe de repli pour une nouvelle vue : premier groupe, sinon « Divers ».",
    fallback_hint = "create a view group first",
    not_found = "view group",
}

impl App {
    /// from == to avant tout require_*, contrairement à move_tag.
    pub fn move_view(
        &self,
        from: ViewGroupId,
        to: ViewGroupId,
        view: ViewId,
    ) -> Result<(), AppError> {
        if from == to {
            return Ok(());
        }
        self.require_view_group(from)?;
        self.require_view_group(to)?;
        if self.store.group_for_view(view)? != Some(from) {
            return Err(AppError::NotFound(format!("view {view} in group {from}")));
        }
        self.store.place_view(to, view, None)?;
        Ok(())
    }
}
