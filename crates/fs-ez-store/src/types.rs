//! Types de requête et d'enregistrement échangés avec les ports.

use fs_ez_core::{
    ApiTokenId, FilterExpr, SessionId, Sort, UserId, VirtualFolderScope, has_unsafe_text,
};
use time::OffsetDateTime;

use crate::error::{StoreError, StoreResult};

/// Longueur maximale de la recherche textuelle, en caractères.
pub const MAX_TEXT_LEN: usize = 200;
/// Taille de page maximale pour toute pagination.
pub const MAX_PAGE_SIZE: usize = 500;
/// Longueur maximale du nom d'un jeton d'API, en caractères.
const MAX_API_TOKEN_NAME_CHARS: usize = 100;

/// Retourne le nom trimé si non vide, ≤ 100 caractères et sans texte à risque.
pub fn normalize_api_token_name(raw: &str) -> Option<String> {
    let name = raw.trim();
    if name.is_empty() || name.chars().count() > MAX_API_TOKEN_NAME_CHARS || has_unsafe_text(raw) {
        return None;
    }
    Some(name.to_string())
}

/// Refuse une taille de page hors de `1..=MAX_PAGE_SIZE`.
pub fn validate_page_limit(limit: usize) -> StoreResult<()> {
    if (1..=MAX_PAGE_SIZE).contains(&limit) {
        Ok(())
    } else {
        Err(StoreError::Invalid(format!(
            "limit must be between 1 and {MAX_PAGE_SIZE}"
        )))
    }
}

/// Erreur `Invalid` avec `message` si `condition`, sinon succès.
fn exclusive(condition: bool, message: &str) -> StoreResult<()> {
    if condition {
        Err(StoreError::Invalid(message.to_string()))
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileListQuery {
    pub filter: Option<FilterExpr>,
    pub untagged: bool,
    pub tagged_since_days: Option<i64>,
    /// Fichiers modifiés depuis `days` jours, sur `file.mtime`.
    pub modified_since_days: Option<i64>,
    /// Recherche sur le nom, insensible à la casse ASCII (fiche 25).
    pub text: Option<String>,
    /// `Root` = fichiers à la racine, `Folder(id)` = membres directs du dossier.
    pub virtual_folder: Option<VirtualFolderScope>,
    pub sort: Sort,
    pub offset: usize,
    /// Borné par l'appelant entre 1 et 500.
    pub limit: usize,
}

impl FileListQuery {
    /// Liste sans filtre ni recherche, offset au début.
    pub fn new(sort: Sort, limit: usize) -> Self {
        Self {
            filter: None,
            untagged: false,
            tagged_since_days: None,
            modified_since_days: None,
            text: None,
            virtual_folder: None,
            sort,
            offset: 0,
            limit,
        }
    }

    /// Fichiers tagués depuis `days` jours.
    pub fn recent(days: i64, sort: Sort, limit: usize) -> Self {
        Self {
            tagged_since_days: Some(days),
            ..Self::new(sort, limit)
        }
    }

    /// Fichiers sans aucun tag.
    pub fn untagged(sort: Sort, limit: usize) -> Self {
        Self {
            untagged: true,
            ..Self::new(sort, limit)
        }
    }

    /// Fichiers modifiés depuis `days` jours.
    pub fn recent_files(days: i64, sort: Sort, limit: usize) -> Self {
        Self {
            modified_since_days: Some(days),
            ..Self::new(sort, limit)
        }
    }

    /// Prédicats exclusifs et texte borné : les implémentations appellent avant exécution.
    pub fn validate(&self) -> StoreResult<()> {
        exclusive(
            self.untagged && self.tagged_since_days.is_some(),
            "untagged and tagged_since_days are mutually exclusive",
        )?;
        exclusive(
            self.tagged_since_days.is_some() && self.modified_since_days.is_some(),
            "tagged_since_days and modified_since_days are mutually exclusive",
        )?;
        exclusive(
            self.untagged && self.modified_since_days.is_some(),
            "untagged and modified_since_days are mutually exclusive",
        )?;
        // Tout FilterExpr porte sur des tags : combiné à untagged il est contradictoire.
        exclusive(
            self.untagged && self.filter.is_some(),
            "untagged cannot be combined with a tag filter",
        )?;
        if let Some(text) = &self.text
            && text.chars().count() > MAX_TEXT_LEN
        {
            return Err(StoreError::Invalid(format!(
                "text exceeds {MAX_TEXT_LEN} characters"
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UserRecord {
    pub id: UserId,
    pub username: String,
    pub password_hash: String,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    /// Incrementee a chaque reset, les sessions d'une generation anterieure sont invalides.
    pub credential_generation: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionRecord {
    pub id: SessionId,
    pub token_hash: String,
    pub created_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
    pub last_seen_at: OffsetDateTime,
    /// Generation du credential utilisateur au moment du login.
    pub credential_generation: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApiTokenRecord {
    pub id: ApiTokenId,
    pub name: String,
    pub token_hash: String,
    pub created_at: OffsetDateTime,
    pub expires_at: Option<OffsetDateTime>,
    pub last_used_at: Option<OffsetDateTime>,
    /// Generation du credential utilisateur au moment de la creation, un reset rend le jeton inerte.
    pub credential_generation: i64,
}

#[cfg(test)]
mod tests;
