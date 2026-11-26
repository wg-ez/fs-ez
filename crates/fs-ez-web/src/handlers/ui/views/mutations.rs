//! Mutations CRUD et duplication des vues ; toute réponse réussie rafraîchit la sidebar.

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Redirect, Response};
use fs_ez_app::{App, AppError, ViewInput};
use fs_ez_core::{SavedView, TagColor, ViewId};

use crate::backend::{Backend, blocking};
use crate::ctx::ViewCreationError;
use crate::error::WebError;
use crate::forms::parse_optional_color;
use crate::handlers::ui::chrome::UiQuery;
use crate::handlers::ui::form::{UiForm, first_value};
use crate::handlers::ui::pages::{FinishAction, Prefs, finish_sidebar, page_base, prefs, render};
use crate::handlers::ui::section;
use crate::handlers::ui::sidebar::build_sidebar;
use crate::handlers::ui::tags::{MutationKind, user_error_message};
use crate::htmx;
use crate::state::AppState;
use crate::views::SidebarTemplate;

use super::form::{input_from_view, parse_group, parse_input};

const VIEWS_CHANGED: &str = "views-changed";
const VIEWS_CHANGED_DATA: &str = "views-changed, data-changed";

/// Ouvre l'éditeur d'une vue dans la sidebar, sans page dédiée.
pub async fn edit_view<B: Backend>(
    State(state): State<AppState<B>>,
    Query(ui): Query<UiQuery>,
    Path(id): Path<ViewId>,
    headers: HeaderMap,
) -> Response {
    let (settings, mut sidebar) = match blocking(state.backend.app(), build_sidebar).await {
        Ok(snapshot) => snapshot,
        Err(error) => return error.into_response(),
    };
    let Prefs { lang, theme } = prefs(&settings, &headers, &ui);
    let Some(view) = sidebar.views.iter().find(|view| view.id == id).cloned() else {
        return WebError::NotFound(format!("view {id}")).into_response();
    };
    sidebar.editing_view = Some(view);
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

pub async fn create_view<B: Backend>(
    State(state): State<AppState<B>>,
    Query(ui): Query<UiQuery>,
    headers: HeaderMap,
    UiForm(pairs): UiForm,
) -> Response {
    let group = parse_group(&pairs);
    let draft = ViewCreationError {
        message: String::new(),
        name: first_value(&pairs, "name").unwrap_or_default().to_string(),
        filter_json: first_value(&pairs, "filter_json")
            .unwrap_or_default()
            .to_string(),
        group: group.as_ref().ok().copied().flatten(),
        color: first_value(&pairs, "color").and_then(TagColor::parse),
    };
    let outcome = match (parse_input(&pairs), group) {
        (Ok(input), Ok(group)) => {
            blocking(state.backend.app(), move |app| {
                app.create_view_in_group(input, group).map(|_| ())
            })
            .await
        }
        (Err(error), _) | (_, Err(error)) => Err(error),
    };
    finish_create(&state, &ui, &headers, outcome, draft).await
}

pub async fn update_view<B: Backend>(
    State(state): State<AppState<B>>,
    Query(ui): Query<UiQuery>,
    Path(id): Path<ViewId>,
    headers: HeaderMap,
    UiForm(pairs): UiForm,
) -> Response {
    let mut input = match parse_input(&pairs) {
        Ok(input) => input,
        Err(error) => return error.into_response(),
    };
    // Formulaire sans champ couleur = conserver l'existante, champ vide = effacer.
    let color_provided = first_value(&pairs, "color").is_some();
    let result = blocking(state.backend.app(), move |app| {
        let existing = app.get_view(id)?;
        if input.name.is_empty() {
            input.name = existing.name.clone();
        }
        if !color_provided {
            input.color = existing.color;
        }
        let input = ViewInput {
            icon: existing.icon,
            ..input
        };
        app.update_view(id, input).map(|_| ())
    })
    .await;
    if let Err(error) = result {
        return error.into_response();
    }
    let target = format!("/views/{id}");
    finish_ok(&state, &ui, &headers, &target, VIEWS_CHANGED_DATA).await
}

pub async fn set_view_color<B: Backend>(
    State(state): State<AppState<B>>,
    Query(ui): Query<UiQuery>,
    Path(id): Path<ViewId>,
    headers: HeaderMap,
    UiForm(pairs): UiForm,
) -> Response {
    let color = match parse_optional_color(first_value(&pairs, "color")) {
        Ok(color) => color,
        Err(error) => return error.into_response(),
    };
    let result = blocking(state.backend.app(), move |app| {
        let mut input = input_from_view(app.get_view(id)?);
        input.color = color;
        app.update_view(id, input).map(|_| ())
    })
    .await;
    if let Err(error) = result {
        return error.into_response();
    }
    finish_ok(&state, &ui, &headers, "/", VIEWS_CHANGED_DATA).await
}

pub async fn rename_view<B: Backend>(
    State(state): State<AppState<B>>,
    Query(ui): Query<UiQuery>,
    Path(id): Path<ViewId>,
    headers: HeaderMap,
    UiForm(pairs): UiForm,
) -> Response {
    let name = first_value(&pairs, "name").unwrap_or_default().to_string();
    let result = blocking(state.backend.app(), move |app| {
        let mut input = input_from_view(app.get_view(id)?);
        input.name = name;
        app.update_view(id, input).map(|_| ())
    })
    .await;
    finish_mutation(&state, &ui, &headers, result).await
}

pub async fn delete_view<B: Backend>(
    State(state): State<AppState<B>>,
    Query(ui): Query<UiQuery>,
    Path(id): Path<ViewId>,
    headers: HeaderMap,
) -> Response {
    let result = blocking(state.backend.app(), move |app| {
        if app.delete_view(id)? {
            Ok(())
        } else {
            Err(AppError::NotFound(format!("view {id}")))
        }
    })
    .await;
    finish_mutation(&state, &ui, &headers, result).await
}

pub async fn duplicate_view<B: Backend>(
    State(state): State<AppState<B>>,
    Query(ui): Query<UiQuery>,
    Path(id): Path<ViewId>,
    headers: HeaderMap,
    UiForm(pairs): UiForm,
) -> Response {
    let requested = first_value(&pairs, "name")
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string);
    let result = blocking(state.backend.app(), move |app| {
        duplicate(app, id, requested.as_deref()).map(|_| ())
    })
    .await;
    finish_mutation(&state, &ui, &headers, result).await
}

fn duplicate(app: &App, id: ViewId, requested: Option<&str>) -> Result<SavedView, AppError> {
    let source = app.get_view(id)?;
    let group = app.store.group_for_view(id)?;
    let name = match requested {
        Some(name) => name.to_string(),
        None => {
            let base = format!("{} copy", source.name);
            unique_name(app, &base)?
        }
    };
    let mut input = input_from_view(source);
    input.name = name;
    input.position = None;
    app.create_view_in_group(input, group)
}

/// Suffixe numérique borné : 1000 tentatives suffisent largement en pratique.
fn unique_name(app: &App, base: &str) -> Result<String, AppError> {
    for index in 2..1000 {
        let candidate = format!("{base} {index}");
        if app.store.get_view_by_name(&candidate)?.is_none() {
            return Ok(candidate);
        }
    }
    Err(AppError::Conflict(
        "too many views with that name".to_string(),
    ))
}

async fn finish_mutation<B: Backend>(
    state: &AppState<B>,
    ui: &UiQuery,
    headers: &HeaderMap,
    outcome: Result<(), WebError>,
) -> Response {
    if let Err(error) = outcome {
        return error.into_response();
    }
    finish_ok(state, ui, headers, "/", VIEWS_CHANGED).await
}

async fn finish_create<B: Backend>(
    state: &AppState<B>,
    ui: &UiQuery,
    headers: &HeaderMap,
    outcome: Result<(), WebError>,
    mut draft: ViewCreationError,
) -> Response {
    let error = match outcome {
        Ok(()) => return finish_ok(state, ui, headers, "/", VIEWS_CHANGED).await,
        Err(error) => error,
    };
    finish_sidebar(state, ui, headers, None, |sidebar, _settings, lang| {
        let Some(message) = user_error_message(state, lang, MutationKind::Other, &error) else {
            return FinishAction::Respond(error.into_response());
        };
        draft.message = message;
        sidebar.view_create_error = Some(draft);
        FinishAction::Continue
    })
    .await
}

async fn finish_ok<B: Backend>(
    state: &AppState<B>,
    ui: &UiQuery,
    headers: &HeaderMap,
    http_redirect: &str,
    trigger: &'static str,
) -> Response {
    if !htmx::is_htmx(headers) {
        return Redirect::to(http_redirect).into_response();
    }
    finish_sidebar(
        state,
        ui,
        headers,
        Some(trigger),
        |_sidebar, _settings, _lang| FinishAction::Continue,
    )
    .await
}
