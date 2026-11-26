//! Section active de la sidebar, calculée à l'identique pour la page et ses fragments.

use axum::http::{HeaderMap, Uri};

use crate::query_params::{FileFilter, FilesParams, ValidatedFiles};

/// Clé de nav de la page courante, vide si l'en-tête HTMX est absent ou hors liste.
pub(crate) fn from_headers(headers: &HeaderMap) -> String {
    headers
        .get("hx-current-url")
        .and_then(|value| value.to_str().ok())
        .and_then(from_url)
        .unwrap_or_default()
}

/// Même calcul que le rendu de page, à partir de l'URL du navigateur.
fn from_url(raw: &str) -> Option<String> {
    let raw = raw.split('#').next().unwrap_or_default();
    let uri: Uri = raw.parse().ok()?;
    if uri.path() == "/settings" {
        return Some("settings".to_string());
    }
    let params = FilesParams::from_scope_url(uri.path_and_query()?.as_str()).ok()?;
    Some(from_params(&params.validate().ok()?))
}

/// Clé de nav d'une liste validée : dossier virtuel prioritaire sur vue, tags et filtres.
pub(crate) fn from_params(params: &ValidatedFiles) -> String {
    if params.virtual_folder.is_some() {
        return "virtual-folders".to_string();
    }
    if let Some(view) = params.view {
        return format!("view:{view}");
    }
    if !params.tags.is_empty() || !params.exclude_tags.is_empty() {
        return "tags".to_string();
    }
    match params.filter {
        Some(FileFilter::Untagged) => "untagged".to_string(),
        Some(FileFilter::Recent) => "recent".to_string(),
        Some(FileFilter::RecentFiles) => "recent-files".to_string(),
        None => "files".to_string(),
    }
}

#[cfg(test)]
mod tests;
