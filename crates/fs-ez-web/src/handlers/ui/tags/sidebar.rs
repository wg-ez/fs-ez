//! Fragments de sidebar : rendu complet et panneau de filtres.

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use fs_ez_core::Settings;

use crate::backend::{Backend, blocking};
use crate::ctx::SidebarData;
use crate::handlers::ui::chrome::UiQuery;
use crate::handlers::ui::pages::{Prefs, page_base, prefs, render};
use crate::handlers::ui::section;
use crate::handlers::ui::sidebar::build_sidebar;
use crate::query_params::{FilesQuery, parse_tag_filters};
use crate::state::AppState;
use crate::views::{SearchFilterPanelTemplate, SidebarTemplate};

pub async fn sidebar_partial<B: Backend>(
    State(state): State<AppState<B>>,
    Query(ui): Query<UiQuery>,
    headers: HeaderMap,
) -> Response {
    let (settings, Prefs { lang, theme }, sidebar) =
        match sidebar_context(&state, &ui, &headers).await {
            Ok(values) => values,
            Err(response) => return *response,
        };
    let base = page_base(
        &state,
        &lang,
        &theme,
        "",
        &section::from_headers(&headers),
        &settings,
    );
    render(SidebarTemplate { base, sidebar })
}

/// Panneau de filtres rechargé à chaque ouverture du menu entonnoir.
pub async fn search_filter_partial<B: Backend>(
    State(state): State<AppState<B>>,
    Query(ui): Query<UiQuery>,
    FilesQuery(params): FilesQuery,
    headers: HeaderMap,
) -> Response {
    let tags = match parse_tag_filters(&params.tags) {
        Ok(tags) => tags,
        Err(error) => return error.into_response(),
    };
    let exclude_tags = match parse_tag_filters(&params.exclude_tags) {
        Ok(tags) => tags,
        Err(error) => return error.into_response(),
    };
    let (settings, Prefs { lang, theme }, sidebar) =
        match sidebar_context(&state, &ui, &headers).await {
            Ok(values) => values,
            Err(response) => return *response,
        };
    let mut base = page_base(&state, &lang, &theme, "", "", &settings);
    base.search_tags = tags;
    base.search_excluded_tags = exclude_tags;
    render(SearchFilterPanelTemplate { base, sidebar })
}

/// Snapshot de sidebar et préférences, erreur convertie en réponse immédiate.
async fn sidebar_context<B: Backend>(
    state: &AppState<B>,
    ui: &UiQuery,
    headers: &HeaderMap,
) -> Result<(Settings, Prefs, SidebarData), Box<Response>> {
    let snapshot = blocking(state.backend.app(), build_sidebar).await;
    let (settings, sidebar) = match snapshot {
        Ok(snapshot) => snapshot,
        Err(error) => return Err(Box::new(error.into_response())),
    };
    let resolved = prefs(&settings, headers, ui);
    Ok((settings, resolved, sidebar))
}
