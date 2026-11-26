//! Parsing commun aux formulaires UI et aux corps JSON : actions de tags et couleurs.

use fs_ez_core::{TagColor, normalize_view_icon};

use crate::error::WebError;

/// Identifiant i64 strictement positif, espaces de bord ignorés.
pub(crate) fn parse_positive_id(raw: &str) -> Option<i64> {
    raw.trim().parse::<i64>().ok().filter(|id| *id > 0)
}

/// Liste d'identifiants séparés par virgules, segments vides ignorés, refus global si invalide.
pub(crate) fn parse_id_list<T: From<i64>>(raw: Option<&str>) -> Result<Vec<T>, WebError> {
    raw.unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            parse_positive_id(part)
                .map(T::from)
                .ok_or_else(|| WebError::BadRequest("invalid id list".to_string()))
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TagAction {
    Add,
    Remove,
    Set,
}

impl TagAction {
    pub(crate) fn parse(raw: &str) -> Result<Self, WebError> {
        match raw {
            "add" => Ok(Self::Add),
            "remove" => Ok(Self::Remove),
            "set" => Ok(Self::Set),
            other => Err(WebError::BadRequest(format!("unknown action {other:?}"))),
        }
    }

    pub(crate) fn parse_optional(raw: Option<&str>) -> Result<Self, WebError> {
        Self::parse(raw.unwrap_or("add"))
    }

    pub(crate) fn apply(
        self,
        app: &fs_ez_app::App,
        file: fs_ez_core::FileId,
        tags: &[String],
    ) -> Result<(), fs_ez_app::AppError> {
        match self {
            Self::Add => app.apply_tags(file, tags, true),
            Self::Remove => app.remove_tags(file, tags),
            Self::Set => app.set_tags(file, tags),
        }
    }

    pub(crate) fn apply_bulk(
        self,
        app: &fs_ez_app::App,
        files: &[fs_ez_core::FileId],
        tags: &[String],
    ) -> Result<usize, fs_ez_app::AppError> {
        match self {
            Self::Add => app.apply_tags_bulk(files, tags, true),
            Self::Remove => app.remove_tags_bulk(files, tags),
            Self::Set => app.set_tags_bulk(files, tags),
        }
    }
}

pub(crate) fn parse_required_color(raw: Option<&str>) -> Result<TagColor, WebError> {
    raw.and_then(TagColor::parse)
        .ok_or_else(|| WebError::BadRequest("invalid color token".to_string()))
}

pub(crate) fn parse_optional_color(raw: Option<&str>) -> Result<Option<TagColor>, WebError> {
    match raw.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => TagColor::parse(value)
            .map(Some)
            .ok_or_else(|| WebError::BadRequest("invalid color token".to_string())),
        None => Ok(None),
    }
}

pub(crate) fn parse_view_icon(raw: Option<String>) -> Result<Option<String>, WebError> {
    match raw {
        Some(raw) => normalize_view_icon(&raw)
            .map_err(|_| WebError::BadRequest("invalid view icon".to_string())),
        None => Ok(None),
    }
}
