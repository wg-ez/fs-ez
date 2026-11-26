//! Cas d'usage des dossiers virtuels : arborescence, déplacements et affectation.

use std::collections::{HashMap, HashSet};

use fs_ez_core::{
    FileId, FileWithTags, VirtualFolder, VirtualFolderId, VirtualFolderMoveError,
    VirtualFolderScope, display_safe, has_unsafe_text, normalize_virtual_folder_name,
    sanitize_component, truncate_utf8, validate_virtual_folder_move,
};
use fs_ez_store::{Page, StoreError, validate_page_limit};

use crate::app::App;
use crate::error::AppError;

/// Nombre maximal de composants d'un chemin virtuel, garde-fou contre une arborescence démesurée.
pub const MAX_VIRTUAL_FOLDER_DEPTH: usize = 64;
/// Longueur maximale d'un nom importé, en octets.
const MAX_IMPORTED_NAME_BYTES: usize = 200;

impl App {
    pub fn list_virtual_folders(&self) -> Result<Vec<VirtualFolder>, AppError> {
        Ok(self.store.list_virtual_folders()?)
    }

    pub fn get_virtual_folder(&self, id: VirtualFolderId) -> Result<VirtualFolder, AppError> {
        self.store
            .get_virtual_folder(id)?
            .ok_or_else(|| AppError::NotFound(format!("virtual folder {}", id.0)))
    }

    /// Enfants directs triés par nom, casse ASCII ignorée.
    pub fn virtual_folder_children(
        &self,
        parent: Option<VirtualFolderId>,
    ) -> Result<Vec<VirtualFolder>, AppError> {
        Ok(self.folder_tree()?.children(parent))
    }

    /// Fenêtre d'enfants directs et total, pour la liste à scroll virtuel.
    pub fn virtual_folder_children_page(
        &self,
        parent: Option<VirtualFolderId>,
        offset: usize,
        limit: usize,
    ) -> Result<(Vec<VirtualFolder>, usize), AppError> {
        let children = self.folder_tree()?.children(parent);
        let total = children.len();
        Ok((
            children.into_iter().skip(offset).take(limit).collect(),
            total,
        ))
    }

    /// Arborescence complète en une seule lecture, index parents et enfants.
    fn folder_tree(&self) -> Result<VirtualFolderTree, AppError> {
        Ok(VirtualFolderTree::new(self.list_virtual_folders()?))
    }

    pub fn create_virtual_folder(
        &self,
        parent: Option<VirtualFolderId>,
        name: &str,
    ) -> Result<VirtualFolder, AppError> {
        let name = folder_name(name)?;
        if let Some(parent) = parent {
            self.get_virtual_folder(parent)?;
        }
        Ok(self.store.create_virtual_folder(parent, &name)?)
    }

    pub fn rename_virtual_folder(
        &self,
        id: VirtualFolderId,
        name: &str,
    ) -> Result<VirtualFolder, AppError> {
        let name = folder_name(name)?;
        Ok(self.store.rename_virtual_folder(id, &name)?)
    }

    /// Déplace un dossier ; un cycle ou un parent inexistant est refusé.
    pub fn move_virtual_folder(
        &self,
        id: VirtualFolderId,
        parent: Option<VirtualFolderId>,
    ) -> Result<VirtualFolder, AppError> {
        if parent.is_some() {
            self.folder_tree()?.validate_move(id, parent)?;
        }
        Ok(self.store.move_virtual_folder(id, parent)?)
    }

    /// Déplace une sélection mixte ; tous les dossiers sont validés avant la moindre écriture.
    pub fn move_selection_to_virtual_folder(
        &self,
        files: &[FileId],
        folders: &[VirtualFolderId],
        target: Option<VirtualFolderId>,
    ) -> Result<usize, AppError> {
        if let Some(target) = target {
            self.get_virtual_folder(target)?;
        }
        if !folders.is_empty() {
            let tree = self.folder_tree()?;
            for id in folders {
                if !tree.contains(*id) {
                    return Err(AppError::NotFound(format!("virtual folder {}", id.0)));
                }
                tree.validate_move(*id, target)?;
            }
        }
        Ok(self.store.move_virtual_selection(files, folders, target)?)
    }

    /// Supprime le sous-arbre avec tous ses fichiers : disque et index d'abord, dossiers ensuite.
    pub fn delete_virtual_folder_tree(
        &self,
        id: VirtualFolderId,
    ) -> Result<crate::files::DeleteReport, AppError> {
        self.get_virtual_folder(id)?;
        let files = self.virtual_folder_files_recursive(id)?;
        let report = self.delete_files(&files)?;
        // FK `ON DELETE CASCADE` détache les fichiers dont l'unlink a échoué, aucun orphelin ne pointe vers un dossier supprimé.
        self.store.delete_virtual_folder(id)?;
        Ok(report)
    }

    /// Suppression groupée idempotente : un sous-dossier déjà emporté par son parent est ignoré.
    pub fn delete_selection(
        &self,
        files: &[FileId],
        folders: &[VirtualFolderId],
    ) -> Result<crate::files::DeleteReport, AppError> {
        let mut report = self.delete_files(files)?;
        for id in folders {
            match self.delete_virtual_folder_tree(*id) {
                Ok(child) => {
                    report.deleted = report.deleted.saturating_add(child.deleted);
                    report.failed = report.failed.saturating_add(child.failed);
                }
                Err(AppError::NotFound(_)) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(report)
    }

    pub fn move_files_to_virtual_folder(
        &self,
        files: &[FileId],
        folder: Option<VirtualFolderId>,
    ) -> Result<usize, AppError> {
        if let Some(folder) = folder {
            self.get_virtual_folder(folder)?;
        }
        Ok(self.store.attach_files_to_folder(files, folder)?)
    }

    /// Page des fichiers directement membres, racine acceptée.
    pub fn files_in_virtual_folder(
        &self,
        folder: Option<VirtualFolderId>,
        offset: usize,
        limit: usize,
    ) -> Result<Page<FileWithTags>, AppError> {
        validate_page_limit(limit)?;
        let mut request = crate::FileListRequest::new(limit);
        request.virtual_folder =
            Some(folder.map_or(VirtualFolderScope::Root, VirtualFolderScope::Folder));
        request.offset = offset;
        Ok(self.list_files_page(&request)?.into_page())
    }

    /// Fichiers du dossier et de tous ses descendants, ordre stable par identifiant.
    pub fn virtual_folder_files_recursive(
        &self,
        id: VirtualFolderId,
    ) -> Result<Vec<FileId>, AppError> {
        let tree = self.folder_tree()?;
        let mut stack = vec![id];
        let mut files = Vec::new();
        let mut seen = HashSet::new();
        while let Some(folder) = stack.pop() {
            if !seen.insert(folder) {
                continue;
            }
            files.extend(self.store.files_in_virtual_folder(Some(folder))?);
            stack.extend(tree.children_ids(folder));
        }
        files.sort();
        files.dedup();
        Ok(files)
    }

    /// Rattache un fichier importé au dossier virtuel de son chemin d'origine, vide = racine.
    pub fn attach_file_to_virtual_path(
        &self,
        id: FileId,
        folder: &[String],
    ) -> Result<(), AppError> {
        if folder.is_empty() {
            return Ok(());
        }
        let folder = self
            .ensure_virtual_folder_path(folder)?
            .ok_or_else(|| AppError::Invalid("empty virtual folder path".to_string()))?;
        self.move_files_to_virtual_folder(&[id], Some(folder))?;
        Ok(())
    }

    /// Crée la chaîne de dossiers d'un chemin importé, `None` si le chemin est vide.
    pub fn ensure_virtual_folder_path(
        &self,
        components: &[String],
    ) -> Result<Option<VirtualFolderId>, AppError> {
        if components.len() > MAX_VIRTUAL_FOLDER_DEPTH {
            return Err(AppError::Invalid(
                "virtual folder path too deep".to_string(),
            ));
        }
        let mut parent = None;
        for component in components {
            let name = imported_name(component);
            let folder = match self.store.find_virtual_folder(parent, &name)? {
                Some(folder) => folder,
                None => match self.store.create_virtual_folder(parent, &name) {
                    Ok(folder) => folder,
                    // Création concurrente du même frère : la ligne existante fait foi.
                    Err(StoreError::Conflict(_)) => self
                        .store
                        .find_virtual_folder(parent, &name)?
                        .ok_or_else(|| AppError::Conflict(format!("virtual folder {name:?}")))?,
                    Err(error) => return Err(error.into()),
                },
            };
            parent = Some(folder.id);
        }
        Ok(parent)
    }
}

/// Arborescence en mémoire : parents pour valider, enfants triés pour naviguer.
struct VirtualFolderTree {
    parents: HashMap<VirtualFolderId, Option<VirtualFolderId>>,
    children: HashMap<Option<VirtualFolderId>, Vec<VirtualFolder>>,
}

impl VirtualFolderTree {
    fn new(folders: Vec<VirtualFolder>) -> Self {
        let mut parents = HashMap::with_capacity(folders.len());
        let mut children: HashMap<Option<VirtualFolderId>, Vec<VirtualFolder>> = HashMap::new();
        for folder in folders {
            parents.insert(folder.id, folder.parent);
            children.entry(folder.parent).or_default().push(folder);
        }
        for siblings in children.values_mut() {
            siblings.sort_by_key(|folder| (folder.name.to_lowercase(), folder.id));
        }
        Self { parents, children }
    }

    fn contains(&self, id: VirtualFolderId) -> bool {
        self.parents.contains_key(&id)
    }

    fn children(&self, parent: Option<VirtualFolderId>) -> Vec<VirtualFolder> {
        self.children.get(&parent).cloned().unwrap_or_default()
    }

    fn children_ids(&self, parent: VirtualFolderId) -> impl Iterator<Item = VirtualFolderId> + '_ {
        self.children
            .get(&Some(parent))
            .into_iter()
            .flatten()
            .map(|folder| folder.id)
    }

    /// Délègue la règle commune et traduit le refus en erreur applicative.
    fn validate_move(
        &self,
        id: VirtualFolderId,
        parent: Option<VirtualFolderId>,
    ) -> Result<(), AppError> {
        validate_virtual_folder_move(&self.parents, id, parent).map_err(|error| match error {
            VirtualFolderMoveError::Invalid(message) => AppError::Invalid(message.to_string()),
            VirtualFolderMoveError::Missing(id) => {
                AppError::NotFound(format!("virtual folder {}", id.0))
            }
        })
    }
}

fn folder_name(raw: &str) -> Result<String, AppError> {
    normalize_virtual_folder_name(raw)
        .ok_or_else(|| AppError::Invalid(format!("invalid virtual folder name {raw:?}")))
}

fn imported_name(raw: &str) -> String {
    let sanitized = sanitize_component(raw.trim());
    let limited = truncate_utf8(&sanitized, MAX_IMPORTED_NAME_BYTES).to_string();
    let limited = if has_unsafe_text(&limited) {
        display_safe(&limited)
    } else {
        limited
    };
    if limited.is_empty() {
        "Dossier".to_string()
    } else {
        limited
    }
}
