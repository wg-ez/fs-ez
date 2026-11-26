//! Liste de fichiers à scroll virtuel : page complète, vue sauvegardée et fragments HTML.

use axum::extract::{Extension, OriginalUri, Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use fs_ez_core::{FileWithTags, SavedView, Settings, Sort, ViewId};
use fs_ez_store::MAX_PAGE_SIZE;

use crate::auth::SessionUser;
use crate::backend::{Backend, blocking};
use crate::ctx::SearchField;
use crate::error::WebError;
use crate::handlers::ui::chrome::{self, UiQuery};
use crate::handlers::ui::pages::{
    Prefs, fallback_prefs, load_settings_and_lang, page_base, prefs, render, render_error,
};
use crate::handlers::ui::section;
use crate::handlers::ui::sidebar::build_sidebar;
use crate::htmx;
use crate::query_params::{
    FilesParams, FilesQuery, ValidatedFiles, build_file_request, sort_token,
};
use crate::state::AppState;
use crate::views::FilesPageTemplate;
use crate::views::FilesRowsTemplate;
use crate::views::IndexTemplate;
use crate::views::files::{FilesData, FilesLabels, build_files_data, build_rows_data};

pub async fn index<B: Backend>(
    State(state): State<AppState<B>>,
    Extension(user): Extension<SessionUser>,
    Query(ui): Query<UiQuery>,
    FilesQuery(params): FilesQuery,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Response {
    render_page(state, user, ui, params, uri, headers, None).await
}

pub async fn view<B: Backend>(
    State(state): State<AppState<B>>,
    Extension(user): Extension<SessionUser>,
    Path(id): Path<ViewId>,
    Query(ui): Query<UiQuery>,
    FilesQuery(params): FilesQuery,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
) -> Response {
    render_page(state, user, ui, params, uri, headers, Some(id)).await
}

pub(crate) async fn render_page<B: Backend>(
    state: AppState<B>,
    user: SessionUser,
    ui: UiQuery,
    params: FilesParams,
    uri: axum::http::Uri,
    headers: HeaderMap,
    view: Option<ViewId>,
) -> Response {
    let params = match params.validate() {
        Ok(mut params) => {
            if let Some(view) = view {
                params.view = Some(view);
            }
            params
        }
        Err(error) => return error.into_response(),
    };

    let fallback = fallback_prefs(&headers, &ui);

    let snapshot = blocking(state.backend.app(), build_sidebar).await;
    let (settings, sidebar) = match snapshot {
        Ok(snapshot) => snapshot,
        Err(error) => {
            tracing::error!(%error, "index snapshot failed");
            return render_error(&state, &fallback.lang, &fallback.theme, &user.username);
        }
    };

    let Prefs { lang, theme } = prefs(&settings, &headers, &ui);
    let loaded = match load_files(&state, params, settings.clone(), true, false).await {
        Ok(loaded) => loaded,
        Err(error) => return error.into_response(),
    };
    let active = section::from_params(&loaded.params);
    let excluded_tags = loaded.params.exclude_tags.clone();
    let fields = search_fields(&loaded.params);
    let files = match load_files_data(&state, &lang, loaded).await {
        Ok(files) => files,
        Err(error) => return error.into_response(),
    };
    let mut base = page_base(&state, &lang, &theme, &user.username, &active, &settings);
    base.search_tags = files.tags.clone();
    base.search_excluded_tags = excluded_tags;
    base.search_fields = fields;
    let q = files.q.clone();
    let template = IndexTemplate {
        base,
        sidebar,
        files,
        q,
        query: chrome::preserved_query(&uri),
    };
    render(template)
}

pub async fn files_partial<B: Backend>(
    State(state): State<AppState<B>>,
    Query(ui): Query<UiQuery>,
    FilesQuery(params): FilesQuery,
    headers: HeaderMap,
) -> Response {
    let (loaded, lang) = match prepare(&state, &headers, &ui, params, true, false).await {
        Ok(prepared) => prepared,
        Err(error) => return error.into_response(),
    };
    let files = match load_files_data(&state, &lang, loaded).await {
        Ok(files) => files,
        Err(error) => return error.into_response(),
    };
    let response = render(FilesPageTemplate { files });
    if response.status().is_success() {
        return htmx::with_vary(response);
    }
    response
}

pub async fn files_rows<B: Backend>(
    State(state): State<AppState<B>>,
    Query(ui): Query<UiQuery>,
    FilesQuery(params): FilesQuery,
    headers: HeaderMap,
) -> Response {
    let (loaded, lang) = match prepare(&state, &headers, &ui, params, false, true).await {
        Ok(prepared) => prepared,
        Err(error) => return error.into_response(),
    };
    let labels = FilesLabels::load(&state.locales, &lang);
    let files = build_rows_data(labels, loaded.items, loaded.params.virtual_folder.is_some());
    let response = render(FilesRowsTemplate { files });
    if response.status().is_success() {
        return htmx::with_vary(response);
    }
    response
}

/// Validation et chargement communs aux fragments avant leur rendu spécifique.
async fn prepare<B: Backend>(
    state: &AppState<B>,
    headers: &HeaderMap,
    ui: &UiQuery,
    params: FilesParams,
    want_total: bool,
    rows: bool,
) -> Result<(Loaded, String), WebError> {
    let params = params.validate()?;
    let (settings, lang) = load_settings_and_lang(state, headers, ui).await?;
    let loaded = load_files(state, params, settings, want_total, rows).await?;
    Ok((loaded, lang))
}

struct Loaded {
    items: Vec<FileWithTags>,
    total: u64,
    view: Option<SavedView>,
    sort: Sort,
    params: ValidatedFiles,
}

async fn load_files<B: Backend>(
    state: &AppState<B>,
    mut params: ValidatedFiles,
    settings: Settings,
    want_total: bool,
    rows: bool,
) -> Result<Loaded, WebError> {
    let page_size = settings.page_size.clamp(1, MAX_PAGE_SIZE);
    let limit = if rows {
        params.limit.unwrap_or(page_size).clamp(1, MAX_PAGE_SIZE)
    } else {
        /* La page SSR ignore offset et limite : elle rend toujours la première page. */
        params.offset = None;
        page_size
    };
    let query_params = params.clone();
    let (items, total, view, sort) = blocking(state.backend.app(), move |app| {
        let request = build_file_request(&query_params, limit, want_total);
        let page = app.list_files_page(&request)?;
        Ok((
            page.items,
            page.total.unwrap_or_default(),
            page.view,
            page.sort,
        ))
    })
    .await?;
    Ok(Loaded {
        items,
        total,
        view,
        sort,
        params,
    })
}

/// Données de liste puis enrichissement de l'explorateur virtuel quand la portée en dépend.
async fn load_files_data<B: Backend>(
    state: &AppState<B>,
    lang: &str,
    loaded: Loaded,
) -> Result<FilesData, WebError> {
    let virtual_folder = loaded.params.virtual_folder;
    let labels = FilesLabels::load(&state.locales, lang);
    let mut files = build_files_data(
        labels,
        loaded.items,
        loaded.total,
        &loaded.params,
        loaded.sort,
        loaded.view.as_ref(),
        virtual_folder,
    );
    if virtual_folder.is_some() {
        crate::handlers::ui::folders::decorate(state, lang, &mut files, virtual_folder).await?;
    }
    Ok(files)
}

/// Tri, filtre et vue repris en champs cachés par la recherche globale.
fn search_fields(params: &ValidatedFiles) -> Vec<SearchField> {
    let mut fields = Vec::new();
    if let Some(filter) = params.filter {
        fields.push(SearchField {
            name: "filter".to_string(),
            value: filter.token().to_string(),
        });
    }
    if let Some(sort) = params.sort {
        fields.push(SearchField {
            name: "sort".to_string(),
            value: sort_token(sort),
        });
    }
    if let Some(view) = params.view {
        fields.push(SearchField {
            name: "view".to_string(),
            value: view.to_string(),
        });
    }
    fields
}
