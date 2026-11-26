//! Vues sauvegardées, tri et groupes de tags.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::error::CoreError;
use crate::filter::FilterExpr;
use crate::ids::{GroupId, ViewGroupId, ViewId};
use crate::tag::TagColor;
use crate::text::normalize_label;

const MAX_VIEW_NAME_CHARS: usize = 100;
const MAX_GROUP_NAME_CHARS: usize = 64;
const MAX_VIEW_ICON_CHARS: usize = 16;

/// Retourne le nom trimé si non vide, ≤ 100 caractères et sans texte à risque.
pub fn normalize_view_name(raw: &str) -> Option<String> {
    normalize_label(raw, MAX_VIEW_NAME_CHARS)
}

/// Retourne le nom trimé si non vide, ≤ 64 caractères et sans texte à risque.
pub fn normalize_group_name(raw: &str) -> Option<String> {
    normalize_label(raw, MAX_GROUP_NAME_CHARS)
}

/// Icône trimée, vide = absente, ≤ 16 caractères et sans texte à risque.
pub fn normalize_view_icon(raw: &str) -> Result<Option<String>, CoreError> {
    if raw.trim().is_empty() {
        return Ok(None);
    }
    normalize_label(raw, MAX_VIEW_ICON_CHARS)
        .map(Some)
        .ok_or_else(|| CoreError::InvalidName(format!("invalid view icon {raw:?}")))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortField {
    Id,
    Name,
    Size,
    Mtime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortDir {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Sort {
    pub field: SortField,
    pub dir: SortDir,
}

impl Default for Sort {
    fn default() -> Self {
        Self {
            field: SortField::Id,
            dir: SortDir::Asc,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedView {
    pub id: ViewId,
    pub name: String,
    pub filter: FilterExpr,
    pub icon: Option<String>,
    pub color: Option<TagColor>,
    pub position: i64,
    pub sort: Sort,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// Deux groupes de schéma identique, seul l'ID typé diffère.
macro_rules! define_group {
    ($name:ident, $id:ty) => {
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        pub struct $name {
            pub id: $id,
            pub name: String,
            pub color: Option<TagColor>,
            pub position: i64,
            pub collapsed: bool,
        }
    };
}

define_group!(TagGroup, GroupId);
define_group!(ViewGroup, ViewGroupId);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewSavedView {
    pub name: String,
    pub filter: FilterExpr,
    pub icon: Option<String>,
    pub color: Option<TagColor>,
    pub position: i64,
    pub sort: Sort,
}

/// Outer `None` = inchangé, `Some(None)` = effacer la valeur nullable.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SavedViewPatch {
    pub name: Option<String>,
    pub filter: Option<FilterExpr>,
    pub icon: Option<Option<String>>,
    pub color: Option<Option<TagColor>>,
    pub position: Option<i64>,
    pub sort: Option<Sort>,
}

#[cfg(test)]
mod tests;
