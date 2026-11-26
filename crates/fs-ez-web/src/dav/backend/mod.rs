//! Backend `DavFileSystem` virtuel : collections SQLite, alias, temporaires atomiques, inbox.

use std::sync::Arc;
use std::time::Duration;

use fs_ez_app::App;
use fs_ez_core::FileId;
use time::OffsetDateTime;

use crate::dav::cache::TtlMap;

mod file;
mod fs;
mod list;
mod meta;
mod open;
mod rename;
mod stream;
mod util;

const PAGE_SIZE: usize = 500;
const TEMP_DIR_MODE: u32 = 0o700;
/// gvfs vérifie le fichier juste après le PUT, l'alias inbox reste résolvable ce laps de temps.
const RECENT_INBOX_TTL: Duration = Duration::from_secs(3600);
const RECENT_INBOX_MAX: usize = 256;

/// Dernier import inbox par nom DAV, pour que le PROPFIND post-PUT retrouve le fichier en bibliothèque.
#[derive(Clone, Debug)]
pub struct RecentInbox(TtlMap<FileId>);

impl Default for RecentInbox {
    fn default() -> Self {
        Self(TtlMap::new(RECENT_INBOX_TTL, RECENT_INBOX_MAX))
    }
}

impl RecentInbox {
    pub fn insert(&self, name: String, id: FileId) {
        self.0.insert(name, id);
    }

    pub fn get(&self, name: &str) -> Option<FileId> {
        self.0.get(name)
    }
}

#[derive(Clone, Debug)]
pub struct VirtualDirs(TtlMap<()>);

impl Default for VirtualDirs {
    fn default() -> Self {
        Self(TtlMap::new(RECENT_INBOX_TTL, RECENT_INBOX_MAX))
    }
}

impl VirtualDirs {
    pub fn insert(&self, key: String) {
        self.0.insert(key, ());
    }

    pub fn contains(&self, key: &str) -> bool {
        self.0.get(key).is_some()
    }

    pub fn remove(&self, key: &str) {
        self.0.remove(key);
    }
}

#[derive(Clone, Debug, Default)]
pub struct DavCredentials {
    /// MOVE de dav-server supprime la destination avant rename, ce droit ne vaut que pour MOVE.
    pub allow_alias_remove: bool,
    /// Seul un DELETE explicite peut emporter le sous-arbre d'un dossier virtuel, jamais la pré-suppression d'un MOVE.
    pub allow_folder_tree_delete: bool,
    /// `X-OC-Mtime` de la requête, propagé au fichier commité.
    pub mtime: Option<OffsetDateTime>,
}

#[derive(Clone)]
pub struct DavFs {
    app: Arc<App>,
    max_upload: u64,
    events: tokio::sync::broadcast::Sender<crate::state::Event>,
    recent_inbox: RecentInbox,
    virtual_dirs: VirtualDirs,
    selection_cache: crate::dav::path::SelectionCache,
}

impl DavFs {
    pub fn new(
        app: Arc<App>,
        max_upload: u64,
        events: tokio::sync::broadcast::Sender<crate::state::Event>,
    ) -> Self {
        Self {
            app,
            max_upload,
            events,
            recent_inbox: RecentInbox::default(),
            virtual_dirs: VirtualDirs::default(),
            selection_cache: crate::dav::path::SelectionCache::default(),
        }
    }
}
