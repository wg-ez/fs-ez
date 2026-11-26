//! Sélection de fichiers commune aux actions en masse.

use std::collections::HashSet;

use fs_ez_app::App;
use fs_ez_core::{FileId, VirtualFolderId};
use fs_ez_store::MAX_PAGE_SIZE;

use crate::error::WebError;
use crate::forms::parse_positive_id;
use crate::handlers::ui::form::first_value;
use crate::query_params::{FilesParams, ValidatedFiles, build_file_request};

/// Sélection explicite des lignes rendues ou toute la requête de la liste.
pub(super) enum Selection {
    Ids(Vec<FileId>),
    /// `excluded` porte les exceptions d'un Ctrl+A désélectionnées côté client.
    Scope {
        params: ValidatedFiles,
        excluded: Vec<FileId>,
    },
}

impl Selection {
    /// Résout la sélection en ids concrets, bornés par `MAX_PAGE_SIZE` par page.
    pub(super) fn resolve(self, app: &App) -> Result<Vec<FileId>, fs_ez_app::AppError> {
        match self {
            Selection::Ids(ids) => Ok(ids),
            Selection::Scope { params, excluded } => scope_ids(app, &params, &excluded),
        }
    }
}

/// Ids positifs d'une clé, dédupliqués, valeur vide ignorée : sélection vide acceptée.
pub(super) fn keyed_ids<T, F>(
    pairs: &[(String, String)],
    key: &str,
    invalid: &str,
    wrap: F,
) -> Result<Vec<T>, WebError>
where
    T: Copy + Eq + std::hash::Hash,
    F: Fn(i64) -> T,
{
    let mut seen = HashSet::new();
    let mut ids = Vec::new();
    for (name, value) in pairs {
        if name != key || value.is_empty() {
            continue;
        }
        match parse_positive_id(value).map(&wrap) {
            Some(id) if seen.insert(id) => ids.push(id),
            Some(_) => {}
            None => return Err(WebError::BadRequest(format!("invalid {invalid} {value:?}"))),
        }
    }
    Ok(ids)
}

fn file_ids(pairs: &[(String, String)], key: &str) -> Result<Vec<FileId>, WebError> {
    keyed_ids(pairs, key, "file id", FileId)
}

/// `oob_ids` restreint le swap aux lignes rendues, repli sur `ids` si le client l'omet.
pub(super) fn out_of_band_ids(pairs: &[(String, String)]) -> Result<Vec<FileId>, WebError> {
    let key = if pairs.iter().any(|(name, _)| name == "oob_ids") {
        "oob_ids"
    } else {
        "ids"
    };
    file_ids(pairs, key)
}

/// `scope=all` couvre toutes les pages, `scope_url` est la première page de la requête.
pub(super) fn parse_selection(pairs: &[(String, String)]) -> Result<Selection, WebError> {
    parse_selection_keyed(pairs, "ids")
}

/// Variante pour une sélection nommée : `file_ids` pour le déplacement mixte.
pub(super) fn parse_selection_keyed(
    pairs: &[(String, String)],
    key: &str,
) -> Result<Selection, WebError> {
    if first_value(pairs, "scope") != Some("all") {
        return file_ids(pairs, key).map(Selection::Ids);
    }
    let raw = first_value(pairs, "scope_url").unwrap_or("/");
    Ok(Selection::Scope {
        params: FilesParams::from_scope_url(raw)?.validate()?,
        excluded: file_ids(pairs, "exclude_ids")?,
    })
}

/// Identifiants de dossiers d'une clé, dédupliqués, valeur vide ignorée.
pub(super) fn parse_folder_ids(
    pairs: &[(String, String)],
    key: &str,
) -> Result<Vec<VirtualFolderId>, WebError> {
    keyed_ids(pairs, key, "virtual folder id", VirtualFolderId)
}

/// Parcourt toutes les pages de la requête : la taille reste bornée par `MAX_PAGE_SIZE`.
fn scope_ids(
    app: &App,
    params: &ValidatedFiles,
    excluded: &[FileId],
) -> Result<Vec<FileId>, fs_ez_app::AppError> {
    let excluded: HashSet<FileId> = excluded.iter().copied().collect();
    let mut ids = Vec::new();
    let mut offset = 0;
    loop {
        let mut request = build_file_request(params, MAX_PAGE_SIZE, false);
        request.offset = offset;
        let page = app.list_files_page(&request)?;
        ids.extend(
            page.items
                .into_iter()
                .map(|item| item.file.id)
                .filter(|id| !excluded.contains(id)),
        );
        match page.next_offset {
            Some(next) => offset = next,
            None => return Ok(ids),
        }
    }
}
