//! Handlers de l'interface : liste de fichiers, shell, page d'erreur et statiques.

pub mod backup;
pub mod chrome;
pub mod file_actions;
pub mod files;
pub mod folders;
pub mod form;
pub mod groups;
pub mod pages;
pub mod preferences;
mod section;
pub mod selection;
pub mod settings;
pub mod sidebar;
pub mod static_files;
pub mod tagging;
pub mod tags;
pub mod view_groups;
pub mod views;
pub mod virtual_folders;

pub use backup::{run_backup, update_backup};
pub use file_actions::{delete_files, rename_file};
pub use files::{files_partial, files_rows, index, view};
pub use groups::{create_group, delete_group, rename_group, reorder_groups, set_group_color};
pub use preferences::{rescan_inbox, toggle_sidebar, update_preferences};
pub use settings::{create_token, revoke_token, settings_page};
pub use static_files::static_file;
pub use tagging::{mutate_bulk, remove_file_tag};
pub use tags::{
    create_tag, delete_tag, place_tag, rename_tag, search_filter_partial, set_tag_color,
    sidebar_partial,
};
pub use view_groups::{
    create_view_group, delete_view_group, place_view, rename_view_group, reorder_view_groups,
    set_view_group_color,
};
pub use views::{
    create_view, delete_view, duplicate_view, edit_view, rename_view, set_view_color, update_view,
};
pub use virtual_folders::{
    create as create_virtual_folder, delete as delete_virtual_folder, delete_selection,
    index as virtual_folders, move_files as move_files_folder, move_folder, move_selection,
    rename as rename_virtual_folder, rows as virtual_folder_rows, tag_folder,
    view as virtual_folder_view,
};
