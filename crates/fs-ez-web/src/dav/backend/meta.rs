//! Métadonnées DAV des collections, alias et fichiers sur disque.

use std::time::SystemTime;

use dav_server::fs::{DavDirEntry, DavMetaData, FsFuture, FsResult};
use fs_ez_core::{FileId, FileRecord};

use super::util::system_time;

#[derive(Clone, Debug)]
pub(super) struct FileMeta {
    len: u64,
    modified: SystemTime,
    dir: bool,
    symlink: bool,
    etag: Option<String>,
}

impl FileMeta {
    pub(super) fn collection() -> Self {
        Self {
            len: 0,
            modified: SystemTime::UNIX_EPOCH,
            dir: true,
            symlink: false,
            etag: None,
        }
    }

    pub(super) fn file(record: &FileRecord) -> Self {
        Self {
            len: record.size,
            modified: system_time(record.mtime),
            dir: false,
            symlink: false,
            etag: Some(make_etag(
                record.id.0,
                record.size,
                record.mtime.unix_timestamp(),
            )),
        }
    }

    pub(super) fn disk(id: Option<FileId>, meta: &std::fs::Metadata) -> Self {
        let len = meta.len();
        let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let etag = match id {
            Some(id) => {
                let secs = modified
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map(|value| value.as_secs())
                    .unwrap_or_default();
                Some(make_etag(id.0, len, secs))
            }
            None => None,
        };
        Self {
            len,
            modified,
            dir: meta.is_dir(),
            symlink: meta.file_type().is_symlink(),
            etag,
        }
    }
}

/// ETag stable identifiant, taille et horodatage, format hexadécimal.
fn make_etag(id: i64, len: u64, stamp: impl std::fmt::LowerHex) -> String {
    format!("{:x}-{:x}-{:x}", id, len, stamp)
}

impl DavMetaData for FileMeta {
    fn len(&self) -> u64 {
        self.len
    }

    fn modified(&self) -> FsResult<SystemTime> {
        Ok(self.modified)
    }

    fn is_dir(&self) -> bool {
        self.dir
    }

    fn is_symlink(&self) -> bool {
        self.symlink
    }

    fn etag(&self) -> Option<String> {
        self.etag.clone()
    }

    fn accessed(&self) -> FsResult<SystemTime> {
        Ok(self.modified)
    }

    fn created(&self) -> FsResult<SystemTime> {
        Ok(self.modified)
    }

    fn status_changed(&self) -> FsResult<SystemTime> {
        Ok(self.modified)
    }
}

#[derive(Debug)]
pub(super) struct Entry {
    pub(super) name: Vec<u8>,
    pub(super) meta: FileMeta,
}

impl DavDirEntry for Entry {
    fn name(&self) -> Vec<u8> {
        self.name.clone()
    }

    fn metadata(&self) -> FsFuture<'_, Box<dyn DavMetaData>> {
        let meta = self.meta.clone();
        Box::pin(async move { Ok(Box::new(meta) as Box<dyn DavMetaData>) })
    }
}
