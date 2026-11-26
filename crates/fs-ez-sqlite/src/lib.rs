#![forbid(unsafe_code)]
//! Adaptateur SQLite : schema, migrations et implementations des ports.

pub mod auth;
pub mod backup;
pub mod convert;
pub mod db;
pub mod files;
mod group_common;
pub mod groups;
pub mod query_sql;
pub mod settings;
pub mod store;
pub mod tags;
pub mod view_groups;
pub mod views;
pub mod virtual_folders;

pub use store::SqliteStore;

#[derive(Debug, thiserror::Error)]
pub enum SqliteError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("migration {0} failed: {1}")]
    Migration(i64, String),
    #[error("unsupported schema version {0} (expected {1})")]
    SchemaVersion(i64, i64),
}

#[cfg(test)]
mod layout_test;
