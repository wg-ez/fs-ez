#![forbid(unsafe_code)]
//! Domaine pur fs-ez : IDs typés, entités, filtres et erreurs.

pub mod codec;
pub mod datetime;
pub mod error;
pub mod file;
pub mod filter;
pub mod ids;
pub mod normalize;
pub mod path;
pub mod settings;
pub mod tag;
pub mod text;
pub mod view;
pub mod virtual_folder;

pub use error::CoreError;
pub use file::{FileKind, FilePatch, FileRecord, Fingerprint, NewFile};
pub use filter::FilterExpr;
pub use ids::{
    ApiTokenId, FileId, GroupId, SessionId, TagId, UserId, ViewGroupId, ViewId, VirtualFolderId,
};
pub use normalize::normalize_tag_name;
pub use path::normalize_relative_path;
pub use settings::{Language, Settings, Theme};
pub use tag::{AppliedTag, FileWithTags, Tag, TagColor, TagCount, validate_hex_color};
pub use text::{
    MAX_VIRTUAL_NAME_BYTES, display_safe, has_unsafe_text, sanitize_component, truncate_utf8,
    unique_child_name, virtual_file_name,
};
pub use view::{
    NewSavedView, SavedView, SavedViewPatch, Sort, SortDir, SortField, TagGroup, ViewGroup,
    normalize_group_name, normalize_view_icon, normalize_view_name,
};
pub use virtual_folder::{
    VirtualFolder, VirtualFolderMoveError, VirtualFolderScope, normalize_virtual_folder_name,
    validate_virtual_folder_move,
};

#[cfg(test)]
mod layout_test;
