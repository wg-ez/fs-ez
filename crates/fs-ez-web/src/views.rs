//! Modèles de templates rendus par les handlers UI.

use askama::Template;

use crate::ctx::{Base, SidebarData};
use crate::views::files::{FileRow, FilesData, FilesRowsData, FolderRowsData};

pub mod files;
pub mod settings;
pub mod tags;

pub use settings::{SettingsPageTemplate, SettingsTokensTemplate};
pub use tags::{SearchFilterPanelTemplate, SidebarOobTemplate, SidebarTemplate};

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub base: Base,
    pub sidebar: SidebarData,
    pub files: FilesData,
    pub q: String,
    pub query: String,
}

/// Fragment autonome pour htmx : ni `<!doctype>`, ni sidebar.
#[derive(Template)]
#[template(path = "partials/files_page.html")]
pub struct FilesPageTemplate {
    pub files: FilesData,
}

/// Fragment de scroll : uniquement les lignes.
#[derive(Template)]
#[template(path = "partials/file_rows.html")]
pub struct FilesRowsTemplate {
    pub files: FilesRowsData,
}

/// Toast de l'édition en masse + pastilles de tags remplacées en OOB.
#[derive(Template)]
#[template(path = "partials/bulk_result.html")]
pub struct BulkResultTemplate {
    pub message: String,
    pub error: bool,
    pub rows: Vec<FileRow>,
}

/// Pastilles de tags d'une ligne, réponse au double-clic de retrait.
#[derive(Template)]
#[template(path = "partials/file_tags_list.html")]
pub struct FileTagsTemplate {
    pub row: FileRow,
}

/// Fragment de scroll des lignes de dossiers.
#[derive(Template)]
#[template(path = "partials/folder_rows.html")]
pub struct FolderRowsTemplate {
    pub folders: FolderRowsData,
}

#[derive(Template)]
#[template(path = "partials/toast.html")]
pub struct ToastTemplate {
    pub message: String,
    pub error: bool,
}

#[derive(Template)]
#[template(path = "error.html")]
pub struct ErrorTemplate {
    pub base: Base,
    pub message: String,
}
