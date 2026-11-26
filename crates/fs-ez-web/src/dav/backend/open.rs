//! Ouverture en lecture et en écriture des alias, temporaires et fichiers DAV.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use dav_server::fs::{DavFile, FsError, OpenOptions};
use fs_ez_app::App;
use fs_ez_core::FileId;
use time::OffsetDateTime;

use super::DavFs;
use super::file::{Commit, CommitTarget, DavFileHandle, ensure_under};
use super::util::{call_app, ensure_parent, ensure_private_parent, io_error, upload_dir_keys};
use crate::dav::logging;
use crate::dav::path::{self, CollectionKind, Node};

impl DavFs {
    fn staged_commit(
        &self,
        target: CommitTarget,
        staging: PathBuf,
        mtime: Option<OffsetDateTime>,
    ) -> Commit {
        Commit::staged(
            Arc::clone(&self.app),
            target,
            staging,
            self.selection_cache.clone(),
            mtime,
        )
    }

    fn staged_handle(
        &self,
        inner: tokio::fs::File,
        id: Option<FileId>,
        commit: Commit,
    ) -> Box<dyn DavFile> {
        Box::new(DavFileHandle::staged(inner, id, commit, self.max_upload))
    }

    pub(super) async fn open_read(
        &self,
        app: &Arc<App>,
        node: &Node,
    ) -> Result<Box<dyn DavFile>, FsError> {
        let (path, id, identity) = match node {
            Node::File { record, .. } => {
                let id = record.id;
                let record = record.clone();
                let target =
                    call_app(Arc::clone(app), move |app| app.read_target_resolved(record)).await?;
                (target.path, Some(id), None)
            }
            Node::Unknown { collection, name } => {
                if collection.kind == CollectionKind::Inbox
                    && let Some(id) = self.recent_inbox.get(name)
                {
                    let target = call_app(Arc::clone(app), move |app| app.read_target(id)).await?;
                    (target.path, Some(id), None)
                } else {
                    (path::temp_path(app, collection, name), None, None)
                }
            }
            Node::FilesEntry { path, dev, ino } => (path.clone(), None, Some((*dev, *ino))),
            Node::Collection(_) => return Err(FsError::Forbidden),
        };
        let inner = match identity {
            Some((dev, ino)) => open_inbox_entry(&path, dev, ino).await?,
            None => tokio::fs::File::open(&path).await.map_err(io_error)?,
        };
        Ok(Box::new(DavFileHandle::new(inner, id, self.max_upload)))
    }

    pub(super) async fn open_write(
        &self,
        app: &Arc<App>,
        node: &Node,
        options: OpenOptions,
        mtime: Option<OffsetDateTime>,
    ) -> Result<Box<dyn DavFile>, FsError> {
        if let Some(size) = options.size {
            ensure_under(self.max_upload, size)?;
        }
        match node {
            Node::File { record, .. } => {
                if options.create_new {
                    return Err(FsError::Exists);
                }
                let id = record.id;
                let staging = path::put_staging_path(app, &id.0.to_string())?;
                ensure_parent(&staging).await?;
                if !options.truncate || options.append {
                    let target = call_app(Arc::clone(app), move |app| app.read_target(id)).await?;
                    let existing = tokio::fs::metadata(&target.path)
                        .await
                        .map_err(io_error)?
                        .len();
                    if !options.truncate {
                        ensure_under(self.max_upload, existing)?;
                    }
                    tokio::fs::copy(&target.path, &staging)
                        .await
                        .map_err(io_error)?;
                }
                let inner = open_for_write(&staging, true, false).await?;
                tracing::info!(
                    target: logging::TARGET,
                    file_id = id.0,
                    partial = !options.truncate,
                    "dav put staging"
                );
                let commit = Commit {
                    events: Some(self.events.clone()),
                    ..self.staged_commit(CommitTarget::Alias(id), staging, mtime)
                };
                Ok(self.staged_handle(inner, Some(id), commit))
            }
            Node::FilesEntry { .. } => Err(FsError::Exists),
            Node::Unknown { collection, name } => {
                if matches!(&collection.kind, CollectionKind::SmartView { .. }) {
                    return Err(FsError::Forbidden);
                }
                if collection.virtual_folder_path().is_some() {
                    return self
                        .open_upload(app, collection, name, options, mtime, Vec::new())
                        .await;
                }
                // Vue : import tagué par les contraintes positives du filtre, aucune contrainte = refus.
                if let CollectionKind::View { view } = &collection.kind {
                    let tags = view.filter.required_tags();
                    if tags.is_empty() {
                        return Err(FsError::Forbidden);
                    }
                    return self
                        .open_upload(app, collection, name, options, mtime, tags)
                        .await;
                }
                let tags = match &collection.kind {
                    CollectionKind::Inbox => Some(Vec::new()),
                    CollectionKind::TagFiles { tags } if options.truncate && !options.append => {
                        Some(path::tag_names(tags))
                    }
                    _ => None,
                };
                if let Some(tags) = tags {
                    return self
                        .open_upload(app, collection, name, options, mtime, tags)
                        .await;
                }
                let temp = path::temp_path(app, collection, name);
                ensure_private_parent(&temp).await?;
                let existing = tokio::fs::metadata(&temp).await;
                if options.create_new && existing.is_ok() {
                    return Err(FsError::Exists);
                }
                if !options.truncate
                    && let Ok(meta) = existing
                {
                    ensure_under(self.max_upload, meta.len())?;
                    let staging = path::put_staging_path(app, name)?;
                    tokio::fs::copy(&temp, &staging).await.map_err(io_error)?;
                    let inner = open_for_write(&staging, false, false).await?;
                    let commit = self.staged_commit(CommitTarget::Temporary(temp), staging, mtime);
                    return Ok(self.staged_handle(inner, None, commit));
                }
                let inner = open_for_write(&temp, true, true).await?;
                tracing::info!(
                    target: logging::TARGET,
                    collection = %collection.key(),
                    temporary = %name,
                    "dav put temporary"
                );
                let commit = self.staged_commit(CommitTarget::Temporary(temp.clone()), temp, mtime);
                Ok(self.staged_handle(inner, None, commit))
            }
            Node::Collection(collection) => {
                let Some(tags) = collection.upload_tags() else {
                    return Err(FsError::Forbidden);
                };
                let Some(name) = collection.components.last() else {
                    return Err(FsError::Forbidden);
                };
                self.open_upload(app, collection, name, options, mtime, tags)
                    .await
            }
        }
    }

    /// PUT inbox ou tag : staging puis import bibliothèque, jamais de ligne inbox.
    async fn open_upload(
        &self,
        app: &Arc<App>,
        collection: &path::Collection,
        name: &str,
        options: OpenOptions,
        mtime: Option<OffsetDateTime>,
        tags: Vec<String>,
    ) -> Result<Box<dyn DavFile>, FsError> {
        if tags.len() > fs_ez_app::tagspec::MAX_TAGS_PER_REQUEST {
            return Err(FsError::Forbidden);
        }
        if options.append || !options.truncate {
            return Err(FsError::Forbidden);
        }
        let staging = path::put_staging_path(app, name)?;
        ensure_private_parent(&staging).await?;
        let inner = open_for_write(&staging, true, true).await?;
        tracing::info!(
            target: logging::TARGET,
            collection = %collection.key(),
            name = %name,
            tags = tags.len(),
            "dav put upload"
        );
        // Dossier virtuel : la collection porte déjà le chemin du parent, le nom reste l'alias.
        let (path, folder, dir_keys) = match collection.virtual_folder_path() {
            Some(folder) => (format!("{}/{}", collection.key(), name), folder, Vec::new()),
            None => {
                let parent_len = collection
                    .components
                    .len()
                    .saturating_sub(usize::from(collection.is_upload_dir()));
                let path = format!("{}/{}", collection.components[..parent_len].join("/"), name);
                let folder = collection.upload_folder(app);
                let dir_keys = upload_dir_keys(
                    &collection.components[..parent_len],
                    collection.upload_base_len(),
                )
                .collect();
                (path, folder, dir_keys)
            }
        };
        let commit = Commit {
            events: Some(self.events.clone()),
            recent_inbox: Some(self.recent_inbox.clone()),
            virtual_dirs: Some(self.virtual_dirs.clone()),
            ..self.staged_commit(
                CommitTarget::Upload {
                    name: name.to_string(),
                    path,
                    folder,
                    dir_keys,
                    tags,
                },
                staging,
                mtime,
            )
        };
        Ok(self.staged_handle(inner, None, commit))
    }
}

/// Ouvre un fichier en écriture, `create` et `truncate` explicites.
async fn open_for_write(
    path: &Path,
    create: bool,
    truncate: bool,
) -> Result<tokio::fs::File, FsError> {
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create(create).truncate(truncate);
    options.open(path).await.map_err(io_error)
}

/// Ouvre un drop inbox sans suivre de symlink, l'inode ouvert doit être celui validé par lstat.
async fn open_inbox_entry(path: &Path, dev: u64, ino: u64) -> Result<tokio::fs::File, FsError> {
    use std::os::unix::fs::MetadataExt;
    let file = tokio::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .await
        .map_err(|error| match error.raw_os_error() {
            Some(libc::ELOOP) => FsError::Forbidden,
            _ => io_error(error),
        })?;
    let meta = file.metadata().await.map_err(io_error)?;
    if !meta.is_file() || meta.dev() != dev || meta.ino() != ino {
        return Err(FsError::Forbidden);
    }
    Ok(file)
}

#[cfg(test)]
mod tests;
