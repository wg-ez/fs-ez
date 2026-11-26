#![forbid(unsafe_code)]
//! Abstraction du système de fichiers, racine et chemins non-UTF8.

pub mod error;
pub mod path;
pub mod pinned;
pub mod root;
pub mod system;

pub use error::FsError;
pub use path::{
    dir_lossy, display_lossy, file_name_lossy, normalize_relative, path_bytes, path_from_bytes,
};
pub use pinned::PinnedDir;
pub use root::Root;
pub use system::{DirEntryInfo, FileMeta, Fs, SystemFs};

#[cfg(test)]
mod layout_test;
