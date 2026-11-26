//! Implémentation `GuardedFileSystem` : lecture, écriture, suppression et promotion.

use std::io;
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

use dav_server::davpath::DavPath;
use dav_server::fs::{
    DavDirEntry, DavFile, DavMetaData, FsError, FsFuture, FsStream, GuardedFileSystem, OpenOptions,
    ReadDirMeta,
};
use fs_ez_app::App;
use fs_ez_core::{FileId, VirtualFolder, normalize_virtual_folder_name};

use super::DavCredentials;
use super::DavFs;
use super::meta::FileMeta;
use super::util::{call_app, ensure_no_failure, io_error, resolve, upload_dir_keys};
use crate::dav::logging;
use crate::dav::path::{self, CollectionKind, Node};

/// Segments décodés, un nom invalide devient `Forbidden` comme aux autres sites DAV.
fn components(path: &DavPath) -> Result<Vec<String>, FsError> {
    path::parse_components(path.as_bytes()).map_err(|_| FsError::Forbidden)
}

/// Clé stable d'un dossier virtuel, segments décodés sans slash initial.
pub(super) fn dir_key(path: &DavPath) -> Option<String> {
    components(path).ok().map(|components| components.join("/"))
}

/// Métadonnées DAV d'un chemin disque, erreurs `io_error` communes.
async fn disk_meta(path: &Path, id: Option<FileId>) -> Result<Box<dyn DavMetaData>, FsError> {
    let meta = tokio::fs::metadata(path).await.map_err(io_error)?;
    Ok(Box::new(FileMeta::disk(id, &meta)))
}

impl DavFs {
    pub(super) fn virtual_contains(&self, path: &DavPath) -> bool {
        dir_key(path).is_some_and(|key| self.virtual_dirs.contains(&key))
    }

    pub(super) async fn node(&self, path: &DavPath) -> Result<Node, FsError> {
        resolve(Arc::clone(&self.app), path, &self.selection_cache).await
    }

    /// Les mutations DAV partagent la connexion SQLite du serveur, `data_version` ne les voit pas.
    pub(super) fn notify_changed(&self) {
        let _ = self.events.send(crate::state::Event::DataChanged);
    }

    /// Invalidation commune aux mutations de dossiers virtuels, log laissé au call-site.
    fn after_virtual_change(&self) {
        self.selection_cache.clear();
        self.notify_changed();
    }

    /// Mémorise les dossiers d'import pour que le PROPFIND suivant les trouve vivants.
    fn remember_upload_dirs(&self, components: &[String], base_len: usize) {
        for key in upload_dir_keys(components, base_len) {
            self.virtual_dirs.insert(key);
        }
    }

    /// MKCOL d'un dossier virtuel : création ou chaînage, puis invalidation et log uniques.
    async fn create_virtual_folder_node(
        &self,
        app: &Arc<App>,
        node: &Node,
    ) -> Result<bool, FsError> {
        let Node::Unknown { collection, name } = node else {
            return Ok(false);
        };
        match &collection.kind {
            CollectionKind::VirtualFolderPath => {
                let mut folder = collection.virtual_folder_path().unwrap_or_default();
                folder.push(name.clone());
                call_app(Arc::clone(app), move |app| {
                    app.ensure_virtual_folder_path(&folder)
                })
                .await?;
            }
            CollectionKind::VirtualFolders => {
                let folder_name = name.clone();
                call_app(Arc::clone(app), move |app| {
                    app.create_virtual_folder(None, &folder_name)
                })
                .await?;
            }
            CollectionKind::VirtualFolder { folder } => {
                let parent = Some(folder.id);
                let folder_name = name.clone();
                call_app(Arc::clone(app), move |app| {
                    app.create_virtual_folder(parent, &folder_name)
                })
                .await?;
            }
            _ => return Ok(false),
        }
        self.after_virtual_change();
        tracing::info!(target: logging::TARGET, name = %name, "dav virtual folder created");
        Ok(true)
    }

    /// MOVE d'un dossier virtuel : déplacement si le parent change, puis renommage si le nom change.
    pub(super) async fn rename_virtual_folder_node(
        &self,
        app: &Arc<App>,
        folder: &VirtualFolder,
        target: &Node,
    ) -> Result<(), FsError> {
        let Node::Unknown { collection, name } = target else {
            return Err(FsError::Forbidden);
        };
        let parent = match &collection.kind {
            CollectionKind::VirtualFolders => None,
            CollectionKind::VirtualFolder { folder } => Some(folder.id),
            _ => return Err(FsError::Forbidden),
        };
        let new_name = normalize_virtual_folder_name(name).ok_or(FsError::Forbidden)?;
        let id = folder.id;
        if folder.parent != parent {
            call_app(Arc::clone(app), move |app| {
                app.move_virtual_folder(id, parent)
            })
            .await?;
        }
        if folder.name != new_name {
            let rename = new_name.clone();
            call_app(Arc::clone(app), move |app| {
                app.rename_virtual_folder(id, &rename)
            })
            .await?;
        }
        self.after_virtual_change();
        tracing::info!(
            target: logging::TARGET, folder = id.0, name = %new_name, "dav virtual folder moved"
        );
        Ok(())
    }
}

impl GuardedFileSystem<DavCredentials> for DavFs {
    fn open<'a>(
        &'a self,
        path: &'a DavPath,
        options: OpenOptions,
        credentials: &'a DavCredentials,
    ) -> FsFuture<'a, Box<dyn DavFile>> {
        Box::pin(async move {
            let app = Arc::clone(&self.app);
            let node = self.node(path).await?;
            if options.write {
                self.open_write(&app, &node, options, credentials.mtime)
                    .await
            } else {
                self.open_read(&app, &node).await
            }
        })
    }

    fn read_dir<'a>(
        &'a self,
        path: &'a DavPath,
        _meta: ReadDirMeta,
        _credentials: &'a DavCredentials,
    ) -> FsFuture<'a, FsStream<Box<dyn DavDirEntry>>> {
        Box::pin(async move { self.read_dir_inner(path).await })
    }

    fn metadata<'a>(
        &'a self,
        path: &'a DavPath,
        _credentials: &'a DavCredentials,
    ) -> FsFuture<'a, Box<dyn DavMetaData>> {
        Box::pin(async move {
            let app = Arc::clone(&self.app);
            let node = self.node(path).await?;
            if let Node::Collection(collection) = &node
                && collection.is_upload_dir()
            {
                if let Some(key) = dir_key(path)
                    && let Some(id) = self.recent_inbox.get(&key)
                {
                    let target = call_app(Arc::clone(&app), move |app| app.read_target(id)).await?;
                    return disk_meta(&target.path, Some(id)).await;
                }
                if !self.virtual_contains(path) {
                    return Err(FsError::NotFound);
                }
            }
            if matches!(&node, Node::Unknown { .. }) && self.virtual_contains(path) {
                return Ok(Box::new(FileMeta::collection()) as Box<dyn DavMetaData>);
            }
            match node {
                Node::Collection(_) => Ok(Box::new(FileMeta::collection()) as Box<dyn DavMetaData>),
                Node::File { record, .. } => {
                    let id = record.id;
                    let target = call_app(Arc::clone(&app), move |app| {
                        app.read_target_resolved(record)
                    })
                    .await?;
                    disk_meta(&target.path, Some(id)).await
                }
                Node::FilesEntry { path, .. } => {
                    let meta = tokio::fs::symlink_metadata(&path).await.map_err(io_error)?;
                    Ok(Box::new(FileMeta::disk(None, &meta)))
                }
                Node::Unknown { collection, name } => {
                    let temp = path::temp_path(&app, &collection, &name);
                    match tokio::fs::metadata(&temp).await {
                        Ok(meta) => Ok(Box::new(FileMeta::disk(None, &meta))),
                        Err(error)
                            if error.kind() == io::ErrorKind::NotFound
                                && collection.kind == CollectionKind::Inbox =>
                        {
                            let id = self.recent_inbox.get(&name).ok_or(FsError::NotFound)?;
                            let target =
                                call_app(Arc::clone(&app), move |app| app.read_target(id)).await?;
                            disk_meta(&target.path, Some(id)).await
                        }
                        Err(error) => Err(io_error(error)),
                    }
                }
            }
        })
    }

    fn symlink_metadata<'a>(
        &'a self,
        path: &'a DavPath,
        credentials: &'a DavCredentials,
    ) -> FsFuture<'a, Box<dyn DavMetaData>> {
        self.metadata(path, credentials)
    }

    fn create_dir<'a>(
        &'a self,
        path: &'a DavPath,
        _credentials: &'a DavCredentials,
    ) -> FsFuture<'a, ()> {
        Box::pin(async move {
            let app = Arc::clone(&self.app);
            let node = self.node(path).await?;
            let components = components(path)?;
            /* MKCOL sous `virtual folders` : nom absent = nouveau dossier, parent déjà résolu. */
            if self.create_virtual_folder_node(&app, &node).await? {
                return Ok(());
            }
            let (folder, base_len) = match &node {
                Node::Collection(collection) if collection.is_upload_dir() => (
                    collection.collection_folder(&app),
                    collection.upload_base_len(),
                ),
                Node::Unknown { collection, name }
                    if matches!(
                        collection.kind,
                        CollectionKind::Inbox | CollectionKind::TagFiles { .. }
                    ) =>
                {
                    let mut folder = collection.collection_folder(&app);
                    folder.push(name.clone());
                    (folder, collection.upload_base_len_for(&components))
                }
                _ => return Err(FsError::Forbidden),
            };
            if folder.is_empty() {
                return Err(FsError::Forbidden);
            }
            let folder_for_app = folder.clone();
            call_app(Arc::clone(&app), move |app| {
                app.ensure_virtual_folder_path(&folder_for_app)
            })
            .await?;
            let key = components.join("/");
            self.remember_upload_dirs(&components, base_len);
            self.virtual_dirs.insert(key);
            Ok(())
        })
    }

    fn remove_dir<'a>(
        &'a self,
        path: &'a DavPath,
        credentials: &'a DavCredentials,
    ) -> FsFuture<'a, ()> {
        Box::pin(async move {
            let components = components(path)?;
            /* DELETE d'un dossier virtuel : même suppression récursive que l'endpoint web. */
            if path::is_virtual_folder_target(&components) {
                // Un MOVE pré-supprime sa destination, seul un DELETE explicite emporte le sous-arbre.
                if !credentials.allow_folder_tree_delete {
                    return Err(FsError::Forbidden);
                }
                let app = Arc::clone(&self.app);
                let node = self.node(path).await?;
                let Node::Collection(collection) = node else {
                    return Err(FsError::NotFound);
                };
                let CollectionKind::VirtualFolder { folder } = collection.kind else {
                    return Err(FsError::Forbidden);
                };
                let id = folder.id;
                let report = call_app(app, move |app| app.delete_virtual_folder_tree(id)).await?;
                ensure_no_failure(report.failed)?;
                self.after_virtual_change();
                tracing::info!(target: logging::TARGET, folder = id.0, "dav virtual folder deleted");
                return Ok(());
            }
            let key = components.join("/");
            if !self.virtual_contains(path) {
                return Err(FsError::Forbidden);
            }
            self.virtual_dirs.remove(&key);
            Ok(())
        })
    }

    fn remove_file<'a>(
        &'a self,
        path: &'a DavPath,
        credentials: &'a DavCredentials,
    ) -> FsFuture<'a, ()> {
        Box::pin(async move {
            let app = Arc::clone(&self.app);
            let node = self.node(path).await?;
            match node {
                Node::Unknown { collection, name } => {
                    if collection.kind == CollectionKind::Inbox {
                        return Err(FsError::Forbidden);
                    }
                    let temp = path::temp_path(&app, &collection, &name);
                    let meta = tokio::fs::metadata(&temp).await.map_err(|error| {
                        if error.kind() == io::ErrorKind::NotFound {
                            FsError::Forbidden
                        } else {
                            io_error(error)
                        }
                    })?;
                    if !meta.is_file() {
                        return Err(FsError::Forbidden);
                    }
                    tokio::fs::remove_file(&temp).await.map_err(io_error)?;
                    tracing::info!(
                        target: logging::TARGET, collection = %collection.key(), temporary = %name,
                        "dav temporary removed"
                    );
                    Ok(())
                }
                Node::File { record, .. } => {
                    if credentials.allow_alias_remove {
                        tracing::info!(
                            target: logging::TARGET, file_id = record.id.0,
                            "dav move replaces alias without deleting file"
                        );
                        Ok(())
                    } else {
                        let id = record.id;
                        let report = call_app(Arc::clone(&app), move |app| {
                            app.delete_resolved_files(&[record])
                        })
                        .await?;
                        ensure_no_failure(report.failed)?;
                        self.notify_changed();
                        tracing::info!(target: logging::TARGET, file_id = id.0, "dav alias deleted");
                        Ok(())
                    }
                }
                Node::Collection(_) => Err(FsError::Forbidden),
                Node::FilesEntry { .. } => Err(FsError::Forbidden),
            }
        })
    }

    fn rename<'a>(
        &'a self,
        from: &'a DavPath,
        to: &'a DavPath,
        _credentials: &'a DavCredentials,
    ) -> FsFuture<'a, ()> {
        Box::pin(async move { self.rename_inner(from, to).await })
    }

    fn copy<'a>(
        &'a self,
        _from: &'a DavPath,
        _to: &'a DavPath,
        _credentials: &'a DavCredentials,
    ) -> FsFuture<'a, ()> {
        Box::pin(async { Err(FsError::Forbidden) })
    }

    fn set_accessed<'a>(
        &'a self,
        _path: &'a DavPath,
        _tm: SystemTime,
        _credentials: &DavCredentials,
    ) -> FsFuture<'a, ()> {
        Box::pin(async { Err(FsError::NotImplemented) })
    }

    fn set_modified<'a>(
        &'a self,
        _path: &'a DavPath,
        _tm: SystemTime,
        _credentials: &DavCredentials,
    ) -> FsFuture<'a, ()> {
        Box::pin(async { Err(FsError::NotImplemented) })
    }
}
