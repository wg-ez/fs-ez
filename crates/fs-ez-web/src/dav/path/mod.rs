//! Résolution des chemins virtuels `/dav` vers tags, vues et alias de fichiers.

mod cache;
mod children;
mod error;
mod model;
mod page;
mod resolve;
mod temp;

pub use cache::SelectionCache;
pub use children::*;
pub use error::fs_error;
pub use model::{
    Collection, CollectionKind, EMPTY_DIR, FILES_DIR, INBOX_DIR, Node, SmartView, TAGS_DIR,
    VIEWS_DIR, VIRTUAL_FOLDERS_DIR, is_virtual_folder_target, tag_names,
};
pub use page::PageQuery;
pub use resolve::*;
pub use temp::{put_staging_path, temp_path};

#[cfg(test)]
mod tests;
