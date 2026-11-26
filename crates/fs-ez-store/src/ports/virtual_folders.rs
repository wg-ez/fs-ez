//! Port de persistance des dossiers virtuels et de l'affectation des fichiers.

use fs_ez_core::{FileId, VirtualFolder, VirtualFolderId};

use crate::error::StoreResult;

pub trait VirtualFolderStore {
    fn list_virtual_folders(&self) -> StoreResult<Vec<VirtualFolder>>;
    fn get_virtual_folder(&self, id: VirtualFolderId) -> StoreResult<Option<VirtualFolder>>;
    /// Dossier de même parent et même nom, comparaison exacte.
    fn find_virtual_folder(
        &self,
        parent: Option<VirtualFolderId>,
        name: &str,
    ) -> StoreResult<Option<VirtualFolder>>;
    fn create_virtual_folder(
        &self,
        parent: Option<VirtualFolderId>,
        name: &str,
    ) -> StoreResult<VirtualFolder>;
    fn rename_virtual_folder(&self, id: VirtualFolderId, name: &str) -> StoreResult<VirtualFolder>;
    fn move_virtual_folder(
        &self,
        id: VirtualFolderId,
        parent: Option<VirtualFolderId>,
    ) -> StoreResult<VirtualFolder>;
    /// Supprime le sous-arbre ; la cascade détache les fichiers encore présents.
    fn delete_virtual_folder(&self, id: VirtualFolderId) -> StoreResult<bool>;
    fn folder_of_file(&self, file: FileId) -> StoreResult<Option<VirtualFolderId>>;
    /// Déplacement groupé atomique : dossiers et fichiers vers une cible unique.
    fn move_virtual_selection(
        &self,
        files: &[FileId],
        folders: &[VirtualFolderId],
        parent: Option<VirtualFolderId>,
    ) -> StoreResult<usize>;
    /// `folder = None` détache de tout dossier, retour au niveau racine.
    fn attach_files_to_folder(
        &self,
        files: &[FileId],
        folder: Option<VirtualFolderId>,
    ) -> StoreResult<usize>;
    /// Identifiants des fichiers directement membres, ordre stable par identifiant.
    fn files_in_virtual_folder(&self, folder: Option<VirtualFolderId>) -> StoreResult<Vec<FileId>>;
}
