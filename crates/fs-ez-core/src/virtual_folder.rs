//! Dossiers virtuels : arborescence d'organisation des fichiers importés.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::ids::VirtualFolderId;
use crate::text::normalize_label;

/// Longueur maximale du nom d'un dossier virtuel, en caractères.
const MAX_NAME_CHARS: usize = 255;

/// Portée d'une liste de fichiers : racine ou dossier virtuel explicite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualFolderScope {
    Root,
    Folder(VirtualFolderId),
}

impl VirtualFolderScope {
    /// Dossier visé, `None` pour la racine.
    pub fn id(self) -> Option<VirtualFolderId> {
        match self {
            Self::Root => None,
            Self::Folder(id) => Some(id),
        }
    }
}

/// Refus d'un déplacement de dossier virtuel, message stable partagé par les deux adaptateurs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualFolderMoveError {
    /// Déplacement interdit, texte prêt pour l'utilisateur.
    Invalid(&'static str),
    /// Dossier absent de l'arbre fourni.
    Missing(VirtualFolderId),
}

/// Valide un déplacement : pas de parent soi-même, pas de sous-arbre, pas de cycle.
pub fn validate_virtual_folder_move(
    tree: &HashMap<VirtualFolderId, Option<VirtualFolderId>>,
    id: VirtualFolderId,
    parent: Option<VirtualFolderId>,
) -> Result<(), VirtualFolderMoveError> {
    let Some(parent) = parent else {
        return Ok(());
    };
    if parent == id {
        return Err(VirtualFolderMoveError::Invalid(
            "a virtual folder cannot contain itself",
        ));
    }
    let mut current = Some(parent);
    let mut seen = HashSet::new();
    while let Some(candidate) = current {
        if candidate == id {
            return Err(VirtualFolderMoveError::Invalid(
                "a virtual folder cannot move into its own subtree",
            ));
        }
        if !seen.insert(candidate) {
            return Err(VirtualFolderMoveError::Invalid(
                "virtual folder cycle detected",
            ));
        }
        current = *tree
            .get(&candidate)
            .ok_or(VirtualFolderMoveError::Missing(candidate))?;
    }
    Ok(())
}

/// Retourne le nom trimé si non vide, ≤ 255 caractères et sans texte à risque.
pub fn normalize_virtual_folder_name(raw: &str) -> Option<String> {
    normalize_label(raw, MAX_NAME_CHARS)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VirtualFolder {
    pub id: VirtualFolderId,
    pub parent: Option<VirtualFolderId>,
    pub name: String,
    pub position: i64,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}
