//! MOVE DAV : promotion de temporaire, renommage d'alias et remplacement de contenu.

use std::sync::Arc;

use dav_server::davpath::DavPath;
use dav_server::fs::FsError;
use time::OffsetDateTime;

use super::DavFs;
use super::util::{call_app, ensure_no_failure, ensure_private_parent, io_error};
use crate::dav::logging;
use crate::dav::path::{self, CollectionKind, Node};

impl DavFs {
    pub(super) async fn rename_inner(&self, from: &DavPath, to: &DavPath) -> Result<(), FsError> {
        let app = Arc::clone(&self.app);
        let source = self.node(from).await?;
        let target = self.node(to).await?;
        if let Node::Collection(collection) = &source
            && let CollectionKind::VirtualFolder { folder } = &collection.kind
        {
            return self.rename_virtual_folder_node(&app, folder, &target).await;
        }
        match (source, target) {
            (
                Node::Unknown {
                    collection: source_collection,
                    name,
                },
                Node::File {
                    collection: target_collection,
                    record,
                },
            ) => {
                ensure_same_collection(&source_collection, &target_collection)?;
                let temp = path::temp_path(&app, &source_collection, &name);
                let mtime = match tokio::fs::metadata(&temp).await {
                    Ok(meta) if meta.is_file() => meta.modified().ok().map(OffsetDateTime::from),
                    _ => return Err(FsError::NotFound),
                };
                let id = record.id;
                let source_path = temp.clone();
                call_app(Arc::clone(&app), move |app| {
                    app.replace_file_contents(id, &source_path, mtime)
                })
                .await?;
                self.notify_changed();
                tracing::info!(
                    target: logging::TARGET,
                    file_id = id.0,
                    temporary = %name,
                    "dav move promoted temporary"
                );
                Ok(())
            }
            (
                Node::File {
                    collection: source_collection,
                    record,
                },
                Node::Unknown {
                    collection: target_collection,
                    name,
                },
            ) => {
                ensure_same_collection(&source_collection, &target_collection)?;
                let id = record.id;
                let new_name = name.clone();
                call_app(Arc::clone(&app), move |app| {
                    app.rename_resolved_file(record, &new_name)
                })
                .await?;
                self.notify_changed();
                tracing::info!(
                    target: logging::TARGET,
                    file_id = id.0,
                    name = %name,
                    "dav alias renamed"
                );
                Ok(())
            }
            (
                Node::File {
                    collection: source_collection,
                    record: source_record,
                },
                Node::File {
                    collection: target_collection,
                    record: target_record,
                },
            ) => {
                ensure_same_collection(&source_collection, &target_collection)?;
                if source_record.id == target_record.id {
                    return Ok(());
                }
                let source_id = source_record.id;
                let target_id = target_record.id;
                let resolved = source_record.clone();
                let source_path = call_app(Arc::clone(&app), move |app| {
                    app.read_target_resolved(resolved)
                })
                .await?
                .path;
                let mtime = tokio::fs::metadata(&source_path)
                    .await
                    .ok()
                    .and_then(|meta| meta.modified().ok())
                    .map(OffsetDateTime::from);
                let staging = path::put_staging_path(&app, &source_id.0.to_string())?;
                ensure_private_parent(&staging).await?;
                tokio::fs::copy(&source_path, &staging)
                    .await
                    .map_err(io_error)?;
                call_app(Arc::clone(&app), move |app| {
                    app.replace_file_contents(target_id, &staging, mtime)
                })
                .await?;
                let report = call_app(Arc::clone(&app), move |app| {
                    app.delete_resolved_files(&[source_record])
                })
                .await?;
                ensure_no_failure(report.failed)?;
                self.notify_changed();
                tracing::info!(
                    target: logging::TARGET,
                    file_id = target_id.0,
                    source_id = source_id.0,
                    "dav move replaced alias and removed source"
                );
                Ok(())
            }
            _ => Err(FsError::Forbidden),
        }
    }
}

/// Inbox interdit des deux côtés puis collection identique, ordre des refus conservé.
fn ensure_same_collection(
    source: &path::Collection,
    target: &path::Collection,
) -> Result<(), FsError> {
    if source.kind == CollectionKind::Inbox || target.kind == CollectionKind::Inbox {
        return Err(FsError::Forbidden);
    }
    if source.key() != target.key() {
        return Err(FsError::Forbidden);
    }
    Ok(())
}
