//! Read model de l'explorateur de dossiers virtuels.

use crate::locales::Locales;

#[derive(Clone)]
pub struct VirtualFoldersLabels {
    pub new_folder: String,
    pub folder_name: String,
    pub create: String,
    pub rename: String,
    pub delete: String,
    pub drop_hint: String,
    pub move_label: String,
    pub move_here: String,
    pub move_cancel: String,
    pub parent: String,
}

impl VirtualFoldersLabels {
    pub fn load(locales: &Locales, lang: &str) -> Self {
        let t = |key: &str| locales.t(lang, key);
        Self {
            new_folder: t("virtual_folders.new"),
            folder_name: t("virtual_folders.name"),
            create: t("virtual_folders.create"),
            rename: t("virtual_folders.rename"),
            delete: t("virtual_folders.delete"),
            drop_hint: t("virtual_folders.drop_hint"),
            move_label: t("virtual_folders.move"),
            move_here: t("virtual_folders.move_here"),
            move_cancel: t("virtual_folders.move_cancel"),
            parent: t("virtual_folders.parent"),
        }
    }
}

pub struct VirtualFolderRow {
    pub id: fs_ez_core::VirtualFolderId,
    pub name: String,
    pub url: String,
    pub rename_action: String,
}

/// Cible de la ligne `..` : identifiant vide quand le parent est la racine.
pub struct VirtualFolderParent {
    pub id: String,
    pub url: String,
}

pub struct VirtualFoldersData {
    pub labels: VirtualFoldersLabels,
    /// Présent hors racine uniquement, absent à la racine.
    pub parent: Option<VirtualFolderParent>,
    /// Fenêtre initiale bornée par `FOLDER_PAGE_SIZE`.
    pub folders: Vec<VirtualFolderRow>,
    /// Nombre total d'enfants directs, fenêtre exclue.
    pub total: usize,
    pub rows_url: String,
    /// Identifiant courant, vide à la racine.
    pub current_id: String,
    pub create_action: String,
    pub create_parent: String,
}

/// Fragment de scroll des dossiers : uniquement les lignes de la fenêtre.
pub struct FolderRowsData {
    pub folders: Vec<VirtualFolderRow>,
    pub labels: VirtualFoldersLabels,
}
