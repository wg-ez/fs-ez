//! Listing des collections DAV : racine, tags, vues, sélections, inbox et dossiers virtuels.

use std::collections::HashSet;
use std::ffi::OsString;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::sync::Arc;

use dav_server::davpath::DavPath;
use dav_server::fs::{DavDirEntry, FsError, FsStream};
use fs_ez_app::App;

use super::DavFs;
use super::meta::{Entry, FileMeta};
use super::stream::{boxed_entries, chain_entry, page_stream, prepend_entries};
use super::util::{call_app, io_error};
use crate::dav::path::{self, CollectionKind, Node, PageQuery, SmartView};

impl DavFs {
    pub(super) async fn read_dir_inner(
        &self,
        path: &DavPath,
    ) -> Result<FsStream<Box<dyn DavDirEntry>>, FsError> {
        let app = Arc::clone(&self.app);
        let node = self.node(path).await?;
        // Inbox racine : liste physique uniquement, les alias d'import récents ne sont jamais des enfants.
        if let Node::Collection(collection) = &node
            && collection.kind == CollectionKind::Inbox
            && !collection.is_upload_dir()
        {
            return inbox_stream(&app).await;
        }
        if self.virtual_contains(path) {
            // Dossier d'import encore vivant côté PUT, ses fichiers sont déjà en bibliothèque.
            return Ok(boxed_entries(Vec::new()));
        }
        let Node::Collection(collection) = node else {
            return Err(FsError::Forbidden);
        };
        if collection.is_upload_dir() {
            return Err(FsError::NotFound);
        }
        match &collection.kind {
            CollectionKind::Root => {
                let mut entries: Vec<Entry> = SmartView::ALL
                    .into_iter()
                    .map(|view| collection_entry(view.name()))
                    .collect();
                entries.extend(
                    [
                        path::TAGS_DIR,
                        path::VIEWS_DIR,
                        path::INBOX_DIR,
                        path::VIRTUAL_FOLDERS_DIR,
                    ]
                    .into_iter()
                    .map(collection_entry),
                );
                Ok(boxed_entries(entries))
            }
            CollectionKind::Tags => {
                let lists = call_app(Arc::clone(&app), path::tag_lists).await?;
                let mut entries = named_entries(lists.used);
                if !lists.empty.is_empty() {
                    entries.push(collection_entry(path::EMPTY_DIR));
                }
                Ok(boxed_entries(entries))
            }
            CollectionKind::Views => {
                let groups = call_app(Arc::clone(&app), |app| app.list_view_groups()).await?;
                let children = path::unique_children(groups, |group| (&group.name, group.id.0));
                Ok(boxed_entries(named_entries(children)))
            }
            CollectionKind::ViewGroup { group } => {
                let group_id = group.id;
                let views =
                    call_app(Arc::clone(&app), move |app| app.views_in_group(group_id)).await?;
                let children = path::unique_children(views, |view| (&view.name, view.id.0));
                Ok(boxed_entries(named_entries(children)))
            }
            CollectionKind::TagSelection { tags } => {
                let lists = call_app(Arc::clone(&app), path::tag_lists).await?;
                let (used, empty) = (lists.used, lists.empty);
                let selected: HashSet<_> = tags.iter().map(|tag| tag.id).collect();
                let names = path::tag_names(tags);
                let lookup = names.clone();
                let matching: HashSet<_> =
                    call_app(Arc::clone(&app), move |app| app.tags_for_selection(&lookup))
                        .await?
                        .into_iter()
                        .map(|tag| tag.id)
                        .collect();
                let remaining: Vec<_> = used
                    .into_iter()
                    .filter(|(_, tag)| !selected.contains(&tag.id) && matching.contains(&tag.id))
                    .collect();
                let empty_selected = empty.iter().any(|(_, tag)| selected.contains(&tag.id));
                let empty_available = empty.iter().any(|(_, tag)| !selected.contains(&tag.id));
                // Fond de sélection : aucun tag à proposer, les fichiers remplacent `All Files`.
                if remaining.is_empty() && !empty_selected {
                    let files = page_stream(app, PageQuery::Tags(names));
                    if !empty_available {
                        return Ok(files);
                    }
                    return Ok(chain_entry(collection_entry(path::EMPTY_DIR), files));
                }
                let mut entries = vec![collection_entry(path::FILES_DIR)];
                entries.extend(named_entries(remaining));
                if empty_available {
                    entries.push(collection_entry(path::EMPTY_DIR));
                }
                Ok(boxed_entries(entries))
            }
            CollectionKind::EmptyTags { tags } => {
                let selected: HashSet<_> = tags.iter().map(|tag| tag.id).collect();
                let lists = call_app(Arc::clone(&app), path::tag_lists).await?;
                let entries = named_entries(
                    lists
                        .empty
                        .into_iter()
                        .filter(|(_, tag)| !selected.contains(&tag.id)),
                );
                Ok(boxed_entries(entries))
            }
            CollectionKind::TagFiles { tags } => {
                Ok(page_stream(app, PageQuery::Tags(path::tag_names(tags))))
            }
            CollectionKind::View { view } => Ok(page_stream(app, PageQuery::View(view.id))),
            CollectionKind::SmartView { view } => {
                Ok(page_stream(app, PageQuery::Mode(view.mode())))
            }
            CollectionKind::Inbox => inbox_stream(&app).await,
            CollectionKind::VirtualFolders => virtual_folder_stream(&app, None).await,
            CollectionKind::VirtualFolder { folder } => {
                virtual_folder_stream(&app, Some(folder.id)).await
            }
            CollectionKind::VirtualFolderPath => Err(FsError::NotFound),
        }
    }
}

/// Entrée de collection sans métadonnées disque, seul le nom varie.
fn collection_entry(name: impl Into<Vec<u8>>) -> Entry {
    Entry {
        name: name.into(),
        meta: FileMeta::collection(),
    }
}

fn named_entries<T>(children: impl IntoIterator<Item = (OsString, T)>) -> Vec<Entry> {
    children
        .into_iter()
        .map(|(name, _)| collection_entry(name.as_bytes().to_vec()))
        .collect()
}

/// Drops directs de l'inbox physique, sans ligne en base, ordre octet déterministe.
async fn inbox_stream(app: &Arc<App>) -> Result<FsStream<Box<dyn DavDirEntry>>, FsError> {
    let dir = path::inbox_dir(app)?;
    let mut entries = Vec::new();
    let mut reader = match tokio::fs::read_dir(&dir).await {
        Ok(reader) => reader,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(boxed_entries(entries)),
        Err(error) => return Err(io_error(error)),
    };
    while let Some(entry) = reader.next_entry().await.map_err(io_error)? {
        let name = entry.file_name();
        let file_type = entry.file_type().await.map_err(io_error)?;
        if !file_type.is_file() {
            continue;
        }
        let meta = entry.metadata().await.map_err(io_error)?;
        entries.push(Entry {
            name: name.as_os_str().as_bytes().to_vec(),
            meta: FileMeta::disk(None, &meta),
        });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(boxed_entries(entries))
}

/// Sous-dossiers puis fichiers directs d'un dossier virtuel, flux paginé.
async fn virtual_folder_stream(
    app: &Arc<App>,
    parent: Option<fs_ez_core::VirtualFolderId>,
) -> Result<FsStream<Box<dyn DavDirEntry>>, FsError> {
    let children = call_app(Arc::clone(app), move |app| {
        path::virtual_folder_children(app, parent)
    })
    .await?;
    let entries = named_entries(children);
    Ok(prepend_entries(
        entries,
        page_stream(Arc::clone(app), PageQuery::Folder(parent)),
    ))
}
