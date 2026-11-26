//! Cas d'usage des groupes de tags : CRUD et placement des tags.

use fs_ez_core::{GroupId, Tag, TagColor, TagGroup, TagId};

use crate::app::App;
use crate::error::AppError;

/// Cas d'usage communs aux groupes de tags et de vues : seuls types et accès au store changent.
macro_rules! group_ops {
    (
        group = $Group:ident,
        item = $Item:ident,
        id = $Id:ident,
        item_id = $ItemId:ident,
        list = $list:ident,
        items_in = $items_in:ident,
        with_items = $with_items:ident,
        one_with_items = $one_with_items:ident,
        create = $create:ident,
        rename = $rename:ident,
        set_color = $set_color:ident,
        set_collapsed = $set_collapsed:ident,
        delete = $delete:ident,
        assign = $assign:ident,
        place = $place:ident,
        prepend = $prepend:ident,
        reorder = $reorder:ident,
        reorder_items = $reorder_items:ident,
        fallback = $fallback:ident,
        require = $require:ident,
        update = $update:ident,
        group_for = $group_for:ident,
        get = $get:ident,
        item_arg = $item_arg:ident: $ItemArg:ty,
        resolve = $resolve:ident,
        delete_doc = $delete_doc:literal,
        place_doc = $place_doc:literal,
        prepend_doc = $prepend_doc:literal,
        fallback_doc = $fallback_doc:literal,
        fallback_hint = $fallback_hint:literal,
        not_found = $not_found:literal,
    ) => {
        impl App {
            pub fn $list(&self) -> Result<Vec<$Group>, AppError> {
                Ok(self.store.$list()?)
            }

            pub fn $items_in(&self, id: $Id) -> Result<Vec<$Item>, AppError> {
                Ok(self.store.$items_in(id)?)
            }

            pub fn $with_items(&self) -> Result<Vec<($Group, Vec<$Item>)>, AppError> {
                let mut groups = Vec::new();
                for group in self.$list()? {
                    let items = self.$items_in(group.id)?;
                    groups.push((group, items));
                }
                Ok(groups)
            }

            pub fn $one_with_items(&self, id: $Id) -> Result<($Group, Vec<$Item>), AppError> {
                let group = self.$require(id)?;
                let items = self.$items_in(id)?;
                Ok((group, items))
            }

            pub fn $create(&self, name: &str, color: Option<TagColor>) -> Result<$Group, AppError> {
                Ok(self.store.$create(name, color)?)
            }

            pub fn $rename(&self, id: $Id, name: &str) -> Result<$Group, AppError> {
                Ok(self.store.$update(id, Some(name), None, None, None)?)
            }

            pub fn $set_color(&self, id: $Id, color: Option<TagColor>) -> Result<$Group, AppError> {
                Ok(self.store.$update(id, None, Some(color), None, None)?)
            }

            pub fn $set_collapsed(&self, id: $Id, collapsed: bool) -> Result<$Group, AppError> {
                Ok(self.store.$update(id, None, None, None, Some(collapsed))?)
            }

            #[doc = $delete_doc]
            pub fn $delete(&self, id: $Id) -> Result<bool, AppError> {
                Ok(self.store.$delete(id)?)
            }

            pub fn $assign(&self, group: $Id, $item_arg: $ItemArg) -> Result<(), AppError> {
                self.$require(group)?;
                let item = self.$resolve($item_arg)?;
                if self.store.$group_for(item.id)? == Some(group) {
                    return Ok(());
                }
                self.store.$place(group, item.id, None)?;
                Ok(())
            }

            #[doc = $place_doc]
            pub fn $place(
                &self,
                group: $Id,
                $item_arg: $ItemArg,
                before: Option<$ItemId>,
            ) -> Result<(), AppError> {
                self.$require(group)?;
                let item = self.$resolve($item_arg)?;
                self.store.$place(group, item.id, before)?;
                Ok(())
            }

            #[doc = $prepend_doc]
            pub fn $prepend(&self, group: $Id, $item_arg: $ItemArg) -> Result<(), AppError> {
                self.$require(group)?;
                let item = self.$resolve($item_arg)?;
                let first = self.$items_in(group)?.into_iter().next();
                self.store
                    .$place(group, item.id, first.map(|member| member.id))?;
                Ok(())
            }

            pub fn $reorder(&self, ids: &[$Id]) -> Result<(), AppError> {
                Ok(self.store.$reorder(ids)?)
            }

            pub fn $reorder_items(&self, group: $Id, ids: &[$ItemId]) -> Result<(), AppError> {
                self.$require(group)?;
                Ok(self.store.$reorder_items(group, ids)?)
            }

            #[doc = $fallback_doc]
            pub fn $fallback(&self) -> Result<$Id, AppError> {
                if let Some(group) = self.$list()?.into_iter().next() {
                    return Ok(group.id);
                }
                match self.$create("Divers", None) {
                    Ok(group) => Ok(group.id),
                    Err(AppError::Conflict(_)) => self
                        .$list()?
                        .into_iter()
                        .next()
                        .map(|group| group.id)
                        .ok_or_else(|| AppError::Conflict($fallback_hint.to_string())),
                    Err(error) => Err(error),
                }
            }

            pub(crate) fn $require(&self, id: $Id) -> Result<$Group, AppError> {
                self.store
                    .$get(id)?
                    .ok_or_else(|| AppError::NotFound(format!(concat!($not_found, " {}"), id.0)))
            }
        }
    };
}

pub(crate) use group_ops;

group_ops! {
    group = TagGroup, item = Tag, id = GroupId, item_id = TagId,
    list = list_groups, items_in = tags_in_group,
    with_items = groups_with_tags, one_with_items = group_with_tags,
    create = create_group, rename = rename_group,
    set_color = set_group_color, set_collapsed = set_group_collapsed,
    delete = delete_group,
    assign = assign_tag, place = place_tag, prepend = prepend_tag,
    reorder = reorder_groups, reorder_items = reorder_group_tags,
    fallback = fallback_group, require = require_group,
    update = update_group, group_for = group_for_tag, get = get_group,
    item_arg = tag: &str, resolve = require_tag,
    delete_doc = "Supprime le groupe et tous ses tags.",
    place_doc = "Place un tag dans un groupe, avant `before` ou en fin de groupe.",
    prepend_doc = "Nouveau tag en tête du groupe, sans déplacer les autres membres.",
    fallback_doc = "Groupe de repli pour un tag implicite : premier groupe, sinon « Divers ».",
    fallback_hint = "create a tag group first",
    not_found = "group",
}

impl App {
    /// require_tag avant le court-circuit from == to, contrairement à move_view.
    pub fn move_tag(&self, from: GroupId, to: GroupId, tag: &str) -> Result<(), AppError> {
        let tag = self.require_tag(tag)?;
        if from == to {
            return Ok(());
        }
        self.require_group(from)?;
        self.require_group(to)?;
        if self.store.group_for_tag(tag.id)? != Some(from) {
            return Err(AppError::NotFound(format!(
                "tag {} in group {from}",
                tag.name
            )));
        }
        self.store.place_tag(to, tag.id, None)?;
        Ok(())
    }
}
