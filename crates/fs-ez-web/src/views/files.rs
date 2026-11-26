//! Read model de la liste de fichiers : lignes, tri et plage de scroll.

mod data;
mod format;
mod labels;
mod urls;
mod virtual_folders;

use fs_ez_core::{FileKind, FileWithTags};

pub use data::{build_files_data, build_rows_data};
pub use format::{format_mtime, format_size};
pub use labels::FilesLabels;
pub use virtual_folders::{
    FolderRowsData, VirtualFolderParent, VirtualFolderRow, VirtualFoldersData, VirtualFoldersLabels,
};

pub struct SortHeader {
    pub key: &'static str,
    pub label: String,
    pub url: String,
    pub active: bool,
    pub sortable: bool,
    pub aria_sort: &'static str,
    pub arrow: &'static str,
}

pub struct TagPill {
    pub name: String,
    pub color: String,
}

pub struct FileRow {
    pub id: fs_ez_core::FileId,
    pub name: String,
    pub size: String,
    pub mtime: String,
    pub kind: &'static str,
    pub tags: Vec<TagPill>,
}

pub struct FilesData {
    pub rows: Vec<FileRow>,
    pub scope_url: String,
    pub total: u64,
    pub q: String,
    pub tags: Vec<String>,
    pub rows_url: String,
    pub refresh_url: String,
    pub refresh_on_tag_change: bool,
    pub sort_headers: Vec<SortHeader>,
    pub labels: FilesLabels,
    /// Lignes déplaçables vers un dossier virtuel.
    pub draggable: bool,
    /// Explorateur affiché uniquement sur les routes de dossiers virtuels.
    pub virtual_folders: Option<VirtualFoldersData>,
}

/// Fragment de scroll : uniquement les lignes rendues.
pub struct FilesRowsData {
    pub rows: Vec<FileRow>,
    pub labels: FilesLabels,
    pub draggable: bool,
}

/// Aucun chemin non-UTF8 ne fait échouer le rendu : `path_display` est déjà lossy.
pub fn rows(items: Vec<FileWithTags>) -> Vec<FileRow> {
    items
        .into_iter()
        .map(|item| {
            let kind = match item.file.kind {
                FileKind::File => "file",
                FileKind::Directory => "directory",
                FileKind::Symlink => "symlink",
            };
            FileRow {
                id: item.file.id,
                name: item.file.name,
                size: format_size(item.file.size),
                mtime: format_mtime(item.file.mtime),
                kind,
                tags: item
                    .tags
                    .into_iter()
                    .map(|applied| TagPill {
                        name: applied.tag.name,
                        color: applied.tag.color.as_token().to_string(),
                    })
                    .collect(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
