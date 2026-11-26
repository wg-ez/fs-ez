//! Poignée de fichier DAV, écriture contrôlée en taille et commit atomique.

use std::io::SeekFrom;
use std::path::PathBuf;
use std::sync::Arc;

use bytes::{Buf, Bytes};
use dav_server::fs::{DavFile, DavMetaData, FsError, FsFuture};
use fs_ez_app::App;
use fs_ez_core::FileId;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

use super::RecentInbox;
use super::meta::FileMeta;
use super::util::{call_app, io_error};
use crate::dav::logging;

pub(super) enum CommitTarget {
    Alias(FileId),
    Temporary(PathBuf),
    Upload {
        name: String,
        /// Chemin DAV complet, distinct du nom seul pour les imports imbriqués.
        path: String,
        /// Dossiers virtuels relatifs à l'inbox ou à la sélection de tags.
        folder: Vec<String>,
        /// Clés DAV des dossiers d'import à garder visibles après le PUT.
        dir_keys: Vec<String>,
        tags: Vec<String>,
    },
}

pub(super) struct Commit {
    pub(super) app: Arc<App>,
    pub(super) target: CommitTarget,
    pub(super) staging: PathBuf,
    pub(super) committed: bool,
    pub(super) events: Option<tokio::sync::broadcast::Sender<crate::state::Event>>,
    pub(super) recent_inbox: Option<RecentInbox>,
    /// Cache de sélection DAV, vidé au commit d'un PUT qui peut ajouter une appartenance.
    pub(super) selection_cache: crate::dav::path::SelectionCache,
    /// Dossiers d'import visibles dans l'arborescence DAV après un PUT imbriqué.
    pub(super) virtual_dirs: Option<super::VirtualDirs>,
    /// `X-OC-Mtime` du PUT, restauré après écriture.
    pub(super) mtime: Option<time::OffsetDateTime>,
}

pub struct DavFileHandle {
    pub(super) inner: tokio::fs::File,
    pub(super) id: Option<FileId>,
    pub(super) cleanup: Option<PathBuf>,
    pub(super) commit: Option<Commit>,
    pub(super) max_upload: u64,
    pub(super) written: u64,
    /// Position courante, bornée par `max_upload` avec les octets écrits.
    pub(super) position: u64,
}

/// Refuse au-delà de la limite d'upload, même variante d'erreur à tous les appels.
pub(super) fn ensure_under(max_upload: u64, len: u64) -> Result<(), FsError> {
    if len > max_upload {
        return Err(FsError::TooLarge);
    }
    Ok(())
}

impl Commit {
    pub(super) fn staged(
        app: Arc<App>,
        target: CommitTarget,
        staging: PathBuf,
        selection_cache: crate::dav::path::SelectionCache,
        mtime: Option<time::OffsetDateTime>,
    ) -> Self {
        Self {
            app,
            target,
            staging,
            committed: false,
            events: None,
            recent_inbox: None,
            selection_cache,
            virtual_dirs: None,
            mtime,
        }
    }
}

impl std::fmt::Debug for DavFileHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DavFileHandle")
            .field("id", &self.id)
            .field("written", &self.written)
            .finish_non_exhaustive()
    }
}

impl Drop for DavFileHandle {
    fn drop(&mut self) {
        if let Some(path) = &self.cleanup {
            let _ = std::fs::remove_file(path);
        }
    }
}

impl DavFile for DavFileHandle {
    fn metadata(&mut self) -> FsFuture<'_, Box<dyn DavMetaData>> {
        Box::pin(async move {
            let meta = self.inner.metadata().await.map_err(io_error)?;
            Ok(Box::new(FileMeta::disk(self.id, &meta)) as Box<dyn DavMetaData>)
        })
    }

    fn write_buf(&mut self, mut buf: Box<dyn Buf + Send>) -> FsFuture<'_, ()> {
        Box::pin(async move {
            while buf.has_remaining() {
                let chunk = buf.chunk().to_vec();
                let len = chunk.len();
                self.write_all_checked(&chunk).await?;
                buf.advance(len);
            }
            Ok(())
        })
    }

    fn write_bytes(&mut self, buf: Bytes) -> FsFuture<'_, ()> {
        Box::pin(async move { self.write_all_checked(&buf).await })
    }

    fn read_bytes(&mut self, count: usize) -> FsFuture<'_, Bytes> {
        Box::pin(async move {
            let mut buffer = vec![0u8; count];
            let read = self.inner.read(&mut buffer).await.map_err(io_error)?;
            buffer.truncate(read);
            Ok(Bytes::from(buffer))
        })
    }

    fn seek(&mut self, pos: SeekFrom) -> FsFuture<'_, u64> {
        Box::pin(async move {
            let position = self.inner.seek(pos).await.map_err(io_error)?;
            ensure_under(self.max_upload, position)?;
            self.position = position;
            Ok(position)
        })
    }

    fn flush(&mut self) -> FsFuture<'_, ()> {
        Box::pin(async move {
            self.inner.flush().await.map_err(io_error)?;
            self.inner.sync_all().await.map_err(io_error)?;
            if let Some(commit) = &mut self.commit
                && !commit.committed
            {
                let staged = self.inner.metadata().await.map_err(io_error)?.len();
                ensure_under(self.max_upload, staged)?;
                let staging = commit.staging.clone();
                let mut imported = false;
                match &commit.target {
                    CommitTarget::Alias(id) => {
                        let app = Arc::clone(&commit.app);
                        let id = *id;
                        let source = staging.clone();
                        let mtime = commit.mtime;
                        call_app(app, move |app| {
                            app.replace_file_contents(id, &source, mtime)
                        })
                        .await?;
                        tracing::info!(
                            target: logging::TARGET,
                            file_id = id.0,
                            bytes = self.written,
                            "dav put committed"
                        );
                    }
                    CommitTarget::Temporary(target) => {
                        if let Some(mtime) = commit.mtime {
                            self.inner
                                .set_times(
                                    tokio::fs::FileTimes::new()
                                        .set_modified(std::time::SystemTime::from(mtime)),
                                )
                                .await
                                .map_err(io_error)?;
                        }
                        if staging != *target {
                            tokio::fs::rename(&staging, target)
                                .await
                                .map_err(io_error)?;
                        }
                        tracing::info!(
                            target: logging::TARGET,
                            bytes = self.written,
                            "dav temporary committed"
                        );
                    }
                    CommitTarget::Upload {
                        name,
                        path,
                        folder,
                        dir_keys,
                        tags,
                    } => {
                        let app = Arc::clone(&commit.app);
                        let name = name.clone();
                        let path = path.clone();
                        let folder = folder.clone();
                        let tags = tags.clone();
                        let source = staging.clone();
                        let upload_name = name.clone();
                        let mtime = commit.mtime;
                        let outcome = call_app(Arc::clone(&app), move |app| {
                            app.import_upload(&source, &upload_name, mtime)
                        })
                        .await?;
                        let indexed = matches!(&outcome, fs_ez_app::LibraryOutcome::Indexed(_));
                        imported = indexed;
                        let file_id = match &outcome {
                            fs_ez_app::LibraryOutcome::Indexed(file) => file.id,
                            fs_ez_app::LibraryOutcome::Duplicate { existing } => existing.id,
                        };
                        // Le PUT exprime la destination : un doublon rejoint aussi ce dossier virtuel.
                        if !folder.is_empty() {
                            let attached = call_app(Arc::clone(&app), move |app| {
                                app.attach_file_to_virtual_path(file_id, &folder)
                            })
                            .await;
                            if let Err(error) = attached {
                                tracing::warn!(
                                    target: logging::TARGET,
                                    file_id = file_id.0,
                                    %error,
                                    "dav virtual folder attach failed"
                                );
                            }
                        }
                        if !tags.is_empty() {
                            call_app(Arc::clone(&app), move |app| {
                                app.apply_tags(file_id, &tags, true)
                            })
                            .await?;
                        }
                        if let Some(recent) = &commit.recent_inbox {
                            recent.insert(name, file_id);
                            recent.insert(path, file_id);
                        }
                        if let Some(dirs) = &commit.virtual_dirs {
                            for key in dir_keys {
                                dirs.insert(key.clone());
                            }
                        }
                        tracing::info!(
                            target: logging::TARGET,
                            bytes = self.written,
                            indexed,
                            "dav upload committed"
                        );
                    }
                }
                if let Some(events) = &commit.events {
                    let _ = events.send(crate::state::Event::DataChanged);
                    if imported {
                        let _ = events.send(crate::state::Event::ImportFinished {
                            imported: 1,
                            duplicates: 0,
                        });
                    }
                }
                commit.committed = true;
                commit.selection_cache.clear();
            }
            self.cleanup = None;
            Ok(())
        })
    }
}

impl DavFileHandle {
    pub(super) fn new(inner: tokio::fs::File, id: Option<FileId>, max_upload: u64) -> Self {
        Self {
            inner,
            id,
            cleanup: None,
            commit: None,
            max_upload,
            written: 0,
            position: 0,
        }
    }

    pub(super) fn staged(
        inner: tokio::fs::File,
        id: Option<FileId>,
        commit: Commit,
        max_upload: u64,
    ) -> Self {
        let mut handle = Self::new(inner, id, max_upload);
        handle.cleanup = Some(commit.staging.clone());
        handle.commit = Some(commit);
        handle
    }

    async fn write_all_checked(&mut self, data: &[u8]) -> Result<(), FsError> {
        let len = data.len() as u64;
        let end = self.position.saturating_add(len);
        ensure_under(self.max_upload, end)?;
        self.inner.write_all(data).await.map_err(io_error)?;
        self.position = end;
        self.written = self.written.saturating_add(len);
        Ok(())
    }
}
