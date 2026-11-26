//! Cas d'usage des fichiers : enregistrement, tagging, renommage, suppression et listes.

mod delete;
mod ensure;
mod list;
mod path;
mod query;
mod rename;
mod shard;
mod tagging;

pub use delete::DeleteReport;
pub use ensure::EnsureOptions;
pub(crate) use ensure::{new_file_from_meta, patch_from_new};
pub use list::{FileListMode, FileListPage, FileListRequest};
pub use shard::{shard_components, shard_dir, sharded_path};
