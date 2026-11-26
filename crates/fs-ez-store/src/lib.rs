#![forbid(unsafe_code)]
//! Ports de persistance fs-ez et erreurs associées, sans dépendance SQLite.

pub mod error;
pub mod page;
pub mod ports;
pub mod types;

pub use error::{STALE_CREDENTIAL_GENERATION, StoreError, StoreResult};
pub use page::Page;
pub use ports::{
    AuthStore, BackupStore, FileStore, GroupStore, SettingsStore, Store, TagStore, ViewGroupStore,
    ViewStore, VirtualFolderStore,
};
pub use types::{
    ApiTokenRecord, FileListQuery, MAX_PAGE_SIZE, MAX_TEXT_LEN, SessionRecord, UserRecord,
    normalize_api_token_name, validate_page_limit,
};

#[cfg(test)]
mod tests;

#[cfg(test)]
mod layout_test;
