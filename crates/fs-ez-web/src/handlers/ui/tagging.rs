//! Mutations de tags : édition en masse et retrait d'une pastille de ligne.

use std::collections::HashSet;

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use fs_ez_app::{App, AppError};
use fs_ez_core::FileId;

use crate::backend::{Backend, blocking};
use crate::error::WebError;
use crate::forms::TagAction;
use crate::handlers::ui::chrome::{self, UiQuery};
use crate::handlers::ui::form::{UiForm, first_value, parse_tags};
use crate::handlers::ui::pages::{redirect_back, render};
use crate::handlers::ui::selection::{
    Selection, out_of_band_ids, parse_folder_ids, parse_selection,
};
use crate::htmx;
use crate::state::AppState;
use crate::views::files::rows;
use crate::views::{BulkResultTemplate, FileTagsTemplate, ToastTemplate};

/// Retrait depuis une pastille du tableau : réponse limitée aux pastilles de la ligne.
pub async fn remove_file_tag<B: Backend>(
    State(state): State<AppState<B>>,
    Path(id): Path<FileId>,
    headers: HeaderMap,
    UiForm(pairs): UiForm,
) -> Response {
    let inputs = match form_tags(&pairs) {
        Ok(inputs) => inputs,
        Err(error) => return WebError::from(error).into_response(),
    };
    let result = blocking(state.backend.app(), move |app| {
        app.remove_tags(id, &inputs)?;
        app.file_with_tags(id)
    })
    .await;
    match result {
        Ok(item) if htmx::is_htmx(&headers) => {
            let Some(row) = rows(vec![item]).pop() else {
                return WebError::Internal("tag row missing".to_string()).into_response();
            };
            with_files_changed(render(FileTagsTemplate { row }))
        }
        Ok(_) => redirect_back(&headers),
        Err(WebError::NotFound(_)) => WebError::NotFound(format!("file {id}")).into_response(),
        Err(error) => error.into_response(),
    }
}

pub async fn mutate_bulk<B: Backend>(
    State(state): State<AppState<B>>,
    Query(ui): Query<UiQuery>,
    headers: HeaderMap,
    UiForm(pairs): UiForm,
) -> Response {
    let action = match TagAction::parse_optional(first_value(&pairs, "action")) {
        Ok(action) => action,
        Err(error) => return error.into_response(),
    };
    let settings = match blocking(state.backend.app(), |app| app.settings()).await {
        Ok(settings) => settings,
        Err(error) => return error.into_response(),
    };
    let lang = chrome::resolve_lang(&headers, &ui, Some(&settings));
    let updated_label = state.locales.t(&lang, "files.bulk.updated");
    let failed_label = state.locales.t(&lang, "files.bulk.failed");

    let selection = match parse_selection(&pairs) {
        Ok(selection) => selection,
        Err(error) => return error.into_response(),
    };
    let folders = match parse_folder_ids(&pairs, "folder_ids") {
        Ok(folders) => folders,
        Err(error) => return error.into_response(),
    };
    let oob_ids = match out_of_band_ids(&pairs) {
        Ok(ids) => ids,
        Err(error) => return error.into_response(),
    };
    if folders.is_empty()
        && let Selection::Ids(ids) = &selection
        && ids.is_empty()
    {
        return WebError::BadRequest("no files selected".to_string()).into_response();
    }
    let inputs = match form_tags(&pairs) {
        Ok(inputs) => inputs,
        Err(error) => {
            if !htmx::is_htmx(&headers) {
                return redirect_back(&headers);
            }
            return render(ToastTemplate {
                message: error.to_string(),
                error: true,
            });
        }
    };

    let counts = match blocking(state.backend.app(), move |app| {
        let mut ids = selection.resolve(app)?;
        for folder in folders {
            ids.extend(app.virtual_folder_files_recursive(folder)?);
        }
        ids.sort();
        ids.dedup();
        if ids.is_empty() {
            return Ok(None);
        }
        bulk_apply(app, &ids, action, &inputs, &oob_ids).map(Some)
    })
    .await
    {
        Ok(Some(counts)) => counts,
        Ok(None) => return WebError::BadRequest("no files selected".to_string()).into_response(),
        Err(error) => return error.into_response(),
    };

    if htmx::is_htmx(&headers) {
        let mut message = format!("{} {}", counts.updated, updated_label);
        if counts.failed > 0 {
            message.push_str(&format!(" · {} {}", counts.failed, failed_label));
        }
        let response = render(BulkResultTemplate {
            message,
            error: counts.updated == 0,
            rows: rows(counts.files),
        });
        with_files_changed(response)
    } else {
        redirect_back(&headers)
    }
}

struct BulkCounts {
    updated: usize,
    failed: usize,
    files: Vec<fs_ez_core::FileWithTags>,
}

/// Une requête d'existence puis une mutation groupée, repli unitaire si le lot échoue.
fn bulk_apply(
    app: &App,
    ids: &[FileId],
    action: TagAction,
    tags: &[String],
    oob_ids: &[FileId],
) -> Result<BulkCounts, AppError> {
    let found = app.store.get_files_by_ids(ids)?;
    let existing: HashSet<FileId> = found.into_iter().map(|file| file.id).collect();
    let existing_ids: Vec<FileId> = ids
        .iter()
        .copied()
        .filter(|id| existing.contains(id))
        .collect();
    let mut counts = BulkCounts {
        updated: 0,
        failed: ids.len() - existing_ids.len(),
        files: Vec::new(),
    };
    if existing_ids.is_empty() {
        return Ok(counts);
    }
    if action.apply_bulk(app, &existing_ids, tags).is_ok() {
        counts.updated = existing_ids.len();
    } else {
        for id in &existing_ids {
            match action.apply(app, *id, tags) {
                Ok(()) => counts.updated += 1,
                Err(_) => counts.failed += 1,
            }
        }
    }
    for id in oob_ids {
        if existing.contains(id)
            && let Ok(item) = app.file_with_tags(*id)
        {
            counts.files.push(item);
        }
    }
    Ok(counts)
}

fn with_files_changed(response: Response) -> Response {
    htmx::with_trigger(response, "files-changed, tags-changed")
}

/// Champ `tags` du formulaire décodé, champ absent = liste vide.
fn form_tags(pairs: &[(String, String)]) -> Result<Vec<String>, AppError> {
    parse_tags(first_value(pairs, "tags").unwrap_or_default())
}
