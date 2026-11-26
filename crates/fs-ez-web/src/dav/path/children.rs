use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

use dav_server::fs::FsError;
use fs_ez_app::{App, AppError};
use fs_ez_core::{
    SavedView, Tag, ViewGroup, ViewGroupId, VirtualFolder, VirtualFolderId, unique_child_name,
};

use super::cache::{SelectionCache, TagLists};
use super::error::fs_error;
use super::model::{EMPTY_DIR, FILES_DIR};

/// Une seule lecture des compteurs, deux espaces de noms séparés et réservés.
pub fn tag_lists(app: &App) -> Result<TagLists, AppError> {
    let tags = app.list_tags()?;
    let mut used_names = reserved_names();
    let mut empty_names = used_names.clone();
    let mut lists = TagLists::default();
    for counted in tags {
        let tag = counted.tag;
        let (names, children) = if counted.count > 0 {
            (&mut used_names, &mut lists.used)
        } else {
            (&mut empty_names, &mut lists.empty)
        };
        let child = unique_child_name(names, &tag.name, tag.id.0);
        children.push((child, tag));
    }
    Ok(lists)
}

fn reserved_names() -> HashSet<OsString> {
    HashSet::from([OsString::from(FILES_DIR), OsString::from(EMPTY_DIR)])
}

fn find_by_name<T>(children: impl IntoIterator<Item = (OsString, T)>, name: &str) -> Option<T> {
    children
        .into_iter()
        .find(|(child, _)| child.as_os_str() == OsStr::new(name))
        .map(|(_, item)| item)
}

/// Enfants aux noms uniques, suffixe d'identifiant si collision.
pub fn unique_children<T>(
    items: impl IntoIterator<Item = T>,
    key: impl Fn(&T) -> (&str, i64),
) -> Vec<(OsString, T)> {
    let mut used = HashSet::new();
    items
        .into_iter()
        .map(|item| {
            let (name, id) = key(&item);
            (unique_child_name(&mut used, name, id), item)
        })
        .collect()
}

/// Résolution d'un segment de tag, `include_empty` ajoute les tags vides après les tags utilisés.
pub fn find_tag(
    app: &App,
    name: &str,
    include_empty: bool,
    cache: &SelectionCache,
) -> Result<Tag, FsError> {
    let lists = cache.tag_lists(app)?;
    if include_empty && let Some(tag) = find_by_name(lists.empty, name) {
        return Ok(tag);
    }
    find_by_name(lists.used, name).ok_or(FsError::NotFound)
}

pub fn find_view_group(app: &App, name: &str) -> Result<ViewGroup, FsError> {
    let groups = app.list_view_groups().map_err(fs_error)?;
    let children = unique_children(groups, |group| (&group.name, group.id.0));
    find_by_name(children, name).ok_or(FsError::NotFound)
}

pub fn find_view_in_group(app: &App, group: ViewGroupId, name: &str) -> Result<SavedView, FsError> {
    let views = app.views_in_group(group).map_err(fs_error)?;
    let children = unique_children(views, |view| (&view.name, view.id.0));
    find_by_name(children, name).ok_or(FsError::NotFound)
}

/// Sous-dossiers aux noms uniques, même convention que tags et vues.
pub fn virtual_folder_children(
    app: &App,
    parent: Option<VirtualFolderId>,
) -> Result<Vec<(OsString, VirtualFolder)>, AppError> {
    let children = app.virtual_folder_children(parent)?;
    Ok(unique_children(children, |folder| {
        (&folder.name, folder.id.0)
    }))
}

pub(super) fn find_virtual_folder_child(
    app: &App,
    parent: Option<VirtualFolderId>,
    name: &str,
) -> Result<Option<VirtualFolder>, FsError> {
    let children = virtual_folder_children(app, parent).map_err(fs_error)?;
    Ok(find_by_name(children, name))
}

pub fn inbox_dir(app: &App) -> Result<PathBuf, FsError> {
    app.inbox_path().map_err(fs_error)
}
