//! Parsing du formulaire de vue en `ViewInput`.

use fs_ez_app::ViewInput;
use fs_ez_core::{SavedView, Sort, ViewGroupId};

use crate::error::WebError;
use crate::forms::{parse_optional_color, parse_positive_id};
use crate::handlers::ui::form::first_value;
use crate::query_params::{parse_sort_arg, trimmed_non_empty};

pub(super) fn parse_input(pairs: &[(String, String)]) -> Result<ViewInput, WebError> {
    let name = first_value(pairs, "name").unwrap_or_default().to_string();
    let sort = match trimmed_non_empty(first_value(pairs, "sort")) {
        Some(raw) => parse_sort_arg(raw)?,
        None => Sort::default(),
    };
    let filter = fs_ez_app::resolve_view_filter(
        first_value(pairs, "filter_json"),
        first_value(pairs, "query"),
    )?;
    Ok(ViewInput {
        name,
        filter,
        icon: None,
        color: parse_optional_color(first_value(pairs, "color"))?,
        position: None,
        sort,
    })
}

/// Entrée d'édition complète d'une vue existante, un seul champ à remplacer.
pub(super) fn input_from_view(view: SavedView) -> ViewInput {
    ViewInput {
        name: view.name,
        filter: Some(view.filter),
        icon: view.icon,
        color: view.color,
        position: Some(view.position),
        sort: view.sort,
    }
}

/// Groupe cible optionnel du formulaire de création de vue.
pub(super) fn parse_group(pairs: &[(String, String)]) -> Result<Option<ViewGroupId>, WebError> {
    match first_value(pairs, "group")
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
    {
        Some(raw) => parse_positive_id(raw)
            .map(|id| Some(ViewGroupId(id)))
            .ok_or_else(|| WebError::BadRequest("invalid group id".to_string())),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests;
