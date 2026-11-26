use fs_ez_app::{App, AppError, FileListMode};
use fs_ez_core::{FileWithTags, ViewId, VirtualFolderId};
use fs_ez_store::Page;

use super::model::{Collection, CollectionKind, tag_names};

/// Requête de page commune à la recherche d'alias et au listing paginé DAV.
#[derive(Clone)]
pub enum PageQuery {
    Tags(Vec<String>),
    View(ViewId),
    Mode(FileListMode),
    /// Dossier virtuel, `None` pour la racine.
    Folder(Option<VirtualFolderId>),
}

impl PageQuery {
    pub fn from_collection(collection: &Collection) -> Option<Self> {
        Some(match &collection.kind {
            CollectionKind::TagFiles { tags } => Self::Tags(tag_names(tags)),
            CollectionKind::View { view } => Self::View(view.id),
            CollectionKind::SmartView { view } => Self::Mode(view.mode()),
            CollectionKind::VirtualFolders => Self::Folder(None),
            CollectionKind::VirtualFolder { folder } => Self::Folder(Some(folder.id)),
            _ => return None,
        })
    }

    pub fn page(
        &self,
        app: &App,
        offset: usize,
        limit: usize,
    ) -> Result<Page<FileWithTags>, AppError> {
        match self {
            Self::Tags(tags) => app.files_for_tags(tags, offset, limit),
            Self::View(id) => app.run_view(*id, offset, limit),
            Self::Mode(mode) => app.files_for_mode(*mode, offset, limit),
            Self::Folder(folder) => app.files_in_virtual_folder(*folder, offset, limit),
        }
    }
}
