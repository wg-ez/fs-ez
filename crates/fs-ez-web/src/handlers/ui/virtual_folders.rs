//! Explorateur de dossiers virtuels : navigation, arborescence et actions.

use axum::extract::{Extension, OriginalUri, Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use fs_ez_app::{App, AppError};
use fs_ez_core::VirtualFolderId;
use fs_ez_store::MAX_PAGE_SIZE;
use serde::Deserialize;

use crate::auth::SessionUser;
use crate::backend::{Backend, blocking};
use crate::error::WebError;
use crate::forms::parse_positive_id;
use crate::handlers::ui::chrome::UiQuery;
use crate::handlers::ui::files::render_page;
/// Fenêtre initiale et taille par défaut des fragments de dossiers.
pub use crate::handlers::ui::folders::FOLDER_PAGE_SIZE;
use crate::handlers::ui::folders::folder_row;
use crate::handlers::ui::form::{UiForm, first_value, parse_tags};
use crate::handlers::ui::pages::{load_settings_and_lang, render};
use crate::handlers::ui::selection::{
    Selection, parse_folder_ids, parse_selection, parse_selection_keyed,
};
use crate::htmx::{self, page_changed};
use crate::query_params::FilesQuery;
use crate::state::AppState;
use crate::views::files::{FolderRowsData, VirtualFoldersLabels};
use crate::views::{FolderRowsTemplate, ToastTemplate};

pub async fn index<B: Backend>(
    State(state): State<AppState<B>>,
    Extension(user): Extension<SessionUser>,
    Query(ui): Query<UiQuery>,
    FilesQuery(mut params): FilesQuery,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Response {
    params.set_virtual_folder_root();
    render_page(state, user, ui, params, uri, headers, None).await
}

pub async fn view<B: Backend>(
    State(state): State<AppState<B>>,
    Extension(user): Extension<SessionUser>,
    Path(id): Path<VirtualFolderId>,
    Query(ui): Query<UiQuery>,
    FilesQuery(mut params): FilesQuery,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Response {
    params.set_virtual_folder(id);
    render_page(state, user, ui, params, uri, headers, None).await
}

#[derive(Debug, Default, Deserialize)]
pub struct FolderRowsQuery {
    pub parent: Option<String>,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

/// Fragment de scroll : uniquement les lignes de dossiers de la fenêtre demandée.
pub async fn rows<B: Backend>(
    State(state): State<AppState<B>>,
    Query(ui): Query<UiQuery>,
    Query(query): Query<FolderRowsQuery>,
    headers: HeaderMap,
) -> Response {
    let parent = match parse_parent(query.parent.as_deref()) {
        Ok(parent) => parent,
        Err(error) => return error.into_response(),
    };
    let offset = query.offset.unwrap_or(0);
    let limit = query
        .limit
        .unwrap_or(FOLDER_PAGE_SIZE)
        .clamp(1, MAX_PAGE_SIZE);
    let (_, lang) = match load_settings_and_lang(&state, &headers, &ui).await {
        Ok(loaded) => loaded,
        Err(error) => return error.into_response(),
    };
    let children = match blocking(state.backend.app(), move |app| {
        app.virtual_folder_children_page(parent, offset, limit)
    })
    .await
    {
        Ok(children) => children.0,
        Err(error) => return error.into_response(),
    };
    let labels = VirtualFoldersLabels::load(&state.locales, &lang);
    let folders = FolderRowsData {
        folders: children.into_iter().map(folder_row).collect(),
        labels,
    };
    let response = render(FolderRowsTemplate { folders });
    if !response.status().is_success() {
        return response;
    }
    htmx::with_vary(response)
}

/// Form value vide = racine, sinon identifiant strictement positif.
fn parse_parent(raw: Option<&str>) -> Result<Option<VirtualFolderId>, WebError> {
    match raw.map(str::trim).filter(|value| !value.is_empty()) {
        None => Ok(None),
        Some(value) => parse_positive_id(value)
            .map(VirtualFolderId)
            .map(Some)
            .ok_or_else(|| WebError::BadRequest("invalid virtual folder".to_string())),
    }
}

/// Nom obligatoire, message identique pour la création et le renommage.
fn required_name(pairs: &[(String, String)]) -> Result<String, WebError> {
    first_value(pairs, "name")
        .map(str::to_string)
        .ok_or_else(|| WebError::BadRequest("missing virtual folder name".to_string()))
}

pub async fn create<B: Backend>(
    State(state): State<AppState<B>>,
    headers: HeaderMap,
    UiForm(pairs): UiForm,
) -> Response {
    let parent = match parse_parent(first_value(&pairs, "parent")) {
        Ok(parent) => parent,
        Err(error) => return error.into_response(),
    };
    let name = match required_name(&pairs) {
        Ok(name) => name,
        Err(error) => return error.into_response(),
    };
    mutate(
        &state,
        &headers,
        move |app| app.create_virtual_folder(parent, &name),
        |_| page_changed(&headers, true, "virtual folder created".to_string()),
    )
    .await
}

pub async fn rename<B: Backend>(
    State(state): State<AppState<B>>,
    Path(id): Path<VirtualFolderId>,
    headers: HeaderMap,
    UiForm(pairs): UiForm,
) -> Response {
    let name = match required_name(&pairs) {
        Ok(name) => name,
        Err(error) => return error.into_response(),
    };
    mutate(
        &state,
        &headers,
        move |app| app.rename_virtual_folder(id, &name),
        |_| page_changed(&headers, true, "virtual folder renamed".to_string()),
    )
    .await
}

pub async fn delete<B: Backend>(
    State(state): State<AppState<B>>,
    Path(id): Path<VirtualFolderId>,
    headers: HeaderMap,
) -> Response {
    mutate(
        &state,
        &headers,
        move |app| app.delete_virtual_folder_tree(id),
        |report| {
            if report.failed == 0 {
                page_changed(&headers, true, "virtual folder deleted".to_string())
            } else {
                page_changed(
                    &headers,
                    false,
                    format!("virtual folder deleted, {} files failed", report.failed),
                )
            }
        },
    )
    .await
}

/// Sélection mixte fichiers + dossiers, erreur de l'un propagée telle quelle.
fn parse_mixed(pairs: &[(String, String)]) -> Result<(Selection, Vec<VirtualFolderId>), WebError> {
    Ok((
        parse_selection_keyed(pairs, "file_ids")?,
        parse_folder_ids(pairs, "folder_ids")?,
    ))
}

/// Supprime une sélection mixte en une requête : fichiers puis sous-arbres de dossiers.
pub async fn delete_selection<B: Backend>(
    State(state): State<AppState<B>>,
    headers: HeaderMap,
    UiForm(pairs): UiForm,
) -> Response {
    let (files, folders) = match parse_mixed(&pairs) {
        Ok(selection) => selection,
        Err(error) => return error.into_response(),
    };
    mutate(
        &state,
        &headers,
        move |app| {
            let files = files.resolve(app)?;
            if files.is_empty() && folders.is_empty() {
                return Ok(None);
            }
            app.delete_selection(&files, &folders).map(Some)
        },
        |report| match report {
            Some(report) if report.failed == 0 => {
                page_changed(&headers, true, "selection deleted".to_string())
            }
            Some(report) => page_changed(
                &headers,
                false,
                format!("selection deleted, {} files failed", report.failed),
            ),
            None => page_changed(&headers, false, "no selection".to_string()),
        },
    )
    .await
}

pub async fn move_folder<B: Backend>(
    State(state): State<AppState<B>>,
    Path(id): Path<VirtualFolderId>,
    headers: HeaderMap,
    UiForm(pairs): UiForm,
) -> Response {
    let parent = match parse_parent(first_value(&pairs, "parent")) {
        Ok(parent) => parent,
        Err(error) => return error.into_response(),
    };
    mutate(
        &state,
        &headers,
        move |app| app.move_virtual_folder(id, parent),
        |_| page_changed(&headers, true, "virtual folder moved".to_string()),
    )
    .await
}

pub async fn move_files<B: Backend>(
    State(state): State<AppState<B>>,
    headers: HeaderMap,
    UiForm(pairs): UiForm,
) -> Response {
    let folder = match parse_parent(first_value(&pairs, "folder")) {
        Ok(folder) => folder,
        Err(error) => return error.into_response(),
    };
    let selection = match parse_selection(&pairs) {
        Ok(selection) => selection,
        Err(error) => return error.into_response(),
    };
    move_into(
        &state,
        &headers,
        move |app| {
            let ids = selection.resolve(app)?;
            if ids.is_empty() {
                return Ok(None);
            }
            app.move_files_to_virtual_folder(&ids, folder).map(Some)
        },
        |count| format!("{count} files moved"),
        "no files selected",
    )
    .await
}

/// Déplace une sélection mixte fichiers + dossiers vers un dossier cible unique.
pub async fn move_selection<B: Backend>(
    State(state): State<AppState<B>>,
    headers: HeaderMap,
    UiForm(pairs): UiForm,
) -> Response {
    let folder = match parse_parent(first_value(&pairs, "folder")) {
        Ok(folder) => folder,
        Err(error) => return error.into_response(),
    };
    let (files, folders) = match parse_mixed(&pairs) {
        Ok(selection) => selection,
        Err(error) => return error.into_response(),
    };
    move_into(
        &state,
        &headers,
        move |app| {
            let files = files.resolve(app)?;
            if files.is_empty() && folders.is_empty() {
                return Ok(None);
            }
            app.move_selection_to_virtual_folder(&files, &folders, folder)
                .map(Some)
        },
        |count| format!("{count} moved"),
        "no selection",
    )
    .await
}

/// Tags appliqués à tous les fichiers du dossier et de ses descendants.
pub async fn tag_folder<B: Backend>(
    State(state): State<AppState<B>>,
    Path(id): Path<VirtualFolderId>,
    headers: HeaderMap,
    UiForm(pairs): UiForm,
) -> Response {
    let raw_tags = first_value(&pairs, "tags").unwrap_or_default();
    let tags = match parse_tags(raw_tags) {
        Ok(tags) => tags,
        Err(error) => return error_response(&headers, error),
    };
    let result = blocking(state.backend.app(), move |app| {
        let files = app.virtual_folder_files_recursive(id)?;
        app.apply_tags_bulk(&files, &tags, true)?;
        Ok(files.len())
    })
    .await;
    match result {
        Ok(count) => {
            let response = page_changed(&headers, true, format!("{count} files tagged"));
            htmx::with_trigger(response, "files-changed, tags-changed")
        }
        Err(error) => error_response(&headers, error),
    }
}

/// Exécute une mutation hors runtime et convertit succès et erreur en réponse htmx.
async fn mutate<B, T, F, S>(
    state: &AppState<B>,
    headers: &HeaderMap,
    operation: F,
    on_success: S,
) -> Response
where
    B: Backend,
    T: Send + 'static,
    F: FnOnce(&App) -> Result<T, AppError> + Send + 'static,
    S: FnOnce(T) -> Response,
{
    match blocking(state.backend.app(), operation).await {
        Ok(value) => on_success(value),
        Err(error) => error_response(headers, error),
    }
}

/// Déplacement vers un dossier cible : toast de compte ou sélection vide.
async fn move_into<B, F>(
    state: &AppState<B>,
    headers: &HeaderMap,
    operation: F,
    moved: impl FnOnce(usize) -> String,
    empty: &str,
) -> Response
where
    B: Backend,
    F: FnOnce(&App) -> Result<Option<usize>, AppError> + Send + 'static,
{
    mutate(state, headers, operation, |count| match count {
        Some(count) => page_changed(headers, true, moved(count)),
        None => page_changed(headers, false, empty.to_string()),
    })
    .await
}

fn error_response(headers: &HeaderMap, error: impl Into<WebError>) -> Response {
    let web = error.into();
    if htmx::is_htmx(headers) {
        return render(ToastTemplate {
            message: web.to_string(),
            error: true,
        });
    }
    web.into_response()
}
