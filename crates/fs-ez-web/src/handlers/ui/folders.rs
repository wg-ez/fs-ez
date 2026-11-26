//! Données de l'explorateur virtuel : dossier courant et sous-dossiers de la liste.

use fs_ez_core::{VirtualFolder, VirtualFolderId, VirtualFolderScope};

use crate::backend::{Backend, blocking};
use crate::error::WebError;
use crate::state::AppState;
use crate::views::files::{
    FilesData, VirtualFolderParent, VirtualFolderRow, VirtualFoldersData, VirtualFoldersLabels,
};

/// Fenêtre initiale et taille par défaut des fragments de dossiers.
pub const FOLDER_PAGE_SIZE: usize = 100;

/// Enrichit la liste de fichiers avec le dossier parent et les sous-dossiers.
pub(crate) async fn decorate<B: Backend>(
    state: &AppState<B>,
    lang: &str,
    files: &mut FilesData,
    folder: Option<VirtualFolderScope>,
) -> Result<(), WebError> {
    let Some(folder) = folder else {
        return Ok(());
    };
    let folder = folder.id();
    let (current, children, total) = blocking(state.backend.app(), move |app| {
        let current = match folder {
            Some(id) => Some(app.get_virtual_folder(id)?),
            None => None,
        };
        let (children, total) = app.virtual_folder_children_page(folder, 0, FOLDER_PAGE_SIZE)?;
        Ok((current, children, total))
    })
    .await?;
    let labels = VirtualFoldersLabels::load(&state.locales, lang);
    files.virtual_folders = Some(build_virtual_folders(
        labels, folder, current, children, total,
    ));
    Ok(())
}

fn folder_url(id: VirtualFolderId) -> String {
    format!("/virtual-folders/{}", id.0)
}

fn parent_url(parent: Option<VirtualFolderId>) -> String {
    parent
        .map(folder_url)
        .unwrap_or_else(|| "/virtual-folders".to_string())
}

/// Racine sans `parent`, dossier courant avec son identifiant.
fn folder_rows_url(folder: Option<VirtualFolderId>) -> String {
    match folder {
        Some(id) => format!("/partials/virtual-folders/rows?parent={}", id.0),
        None => "/partials/virtual-folders/rows".to_string(),
    }
}

pub(crate) fn folder_row(child: VirtualFolder) -> VirtualFolderRow {
    let id = child.id;
    VirtualFolderRow {
        id,
        name: child.name,
        url: folder_url(id),
        rename_action: format!("/partials/virtual-folders/{}/rename", id.0),
    }
}

fn build_virtual_folders(
    labels: VirtualFoldersLabels,
    folder: Option<VirtualFolderId>,
    current: Option<VirtualFolder>,
    children: Vec<VirtualFolder>,
    total: usize,
) -> VirtualFoldersData {
    let parent = current.map(|current| VirtualFolderParent {
        id: current
            .parent
            .map(|parent| parent.0.to_string())
            .unwrap_or_default(),
        url: parent_url(current.parent),
    });
    let current_id = folder
        .map(|folder| folder.0.to_string())
        .unwrap_or_default();
    VirtualFoldersData {
        labels,
        parent,
        folders: children.into_iter().map(folder_row).collect(),
        total,
        rows_url: folder_rows_url(folder),
        create_action: "/partials/virtual-folders".to_string(),
        create_parent: current_id.clone(),
        current_id,
    }
}
