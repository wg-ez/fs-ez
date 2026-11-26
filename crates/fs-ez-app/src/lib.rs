#![forbid(unsafe_code)]
//! Cas d'usage fs-ez et read models pour l'UI.

pub mod app;
pub mod auth;
pub mod backup;
pub mod config;
pub mod content;
pub mod error;
pub mod files;
pub mod groups;
pub mod import;
pub mod maintenance;
pub mod owner;
pub mod password;
pub mod settings;
pub mod staging;
pub mod status;
pub mod tags;
pub mod tagspec;
pub mod token;
pub mod view_groups;
pub mod views;
pub mod virtual_folders;

pub use app::App;
pub use auth::{CreatedApiToken, LoginSession, Principal};
pub use backup::BackupReport;
pub use config::{
    DEFAULT_GID, DEFAULT_UID, FileConfig, FileOwner, env_non_empty, file_owner, first_some,
    load_file_config,
};
pub use content::ReadTarget;
pub use error::AppError;
pub use files::{
    EnsureOptions, FileListMode, FileListPage, FileListRequest, shard_components, shard_dir,
    sharded_path,
};
pub use import::{ImportCounts, ImportReport, LibraryOutcome, is_temporary, sanitize_filename};
pub use maintenance::{InitReport, RepairReport};
pub use owner::OwnerReport;
pub use password::MIN_PASSWORD_LEN;
pub use status::StatusReport;
pub use tags::random_tag_color;
pub use tagspec::{parse_tag_name, parse_tag_names};
pub use views::{ViewInput, coherent_filter, parse_filter_query, resolve_view_filter};

#[cfg(test)]
mod layout_test;
