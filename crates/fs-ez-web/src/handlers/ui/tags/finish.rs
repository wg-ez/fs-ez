//! Fin de mutation : erreurs localisées, sidebar OOB et déclencheurs HTMX.

use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use fs_ez_core::GroupId;

use crate::backend::Backend;
use crate::ctx::{CreationError, ViewGroupCreationError};
use crate::error::WebError;
use crate::handlers::ui::chrome::UiQuery;
use crate::handlers::ui::pages::{FinishAction, finish_sidebar};
use crate::htmx;
use crate::state::AppState;

/// `data-changed` rafraîchit aussi la liste de fichiers quand un tag change.
const TAGS_CHANGED: &str = "tags-changed, data-changed";
/// Même rafraîchissement pour les vues, sans toucher aux fichiers.
const VIEWS_CHANGED: &str = "views-changed, data-changed";

#[derive(Clone, Copy)]
pub(crate) enum MutationKind {
    Tag,
    Group,
    ViewGroup,
    Other,
}

#[derive(Clone, Copy)]
enum CreationTarget {
    Group,
    ViewGroup,
    Tag(GroupId),
}

pub(crate) async fn finish<B: Backend>(
    state: &AppState<B>,
    ui: &UiQuery,
    headers: &HeaderMap,
    outcome: Result<(), WebError>,
) -> Response {
    finish_kind(state, ui, headers, outcome, MutationKind::Other, None, "").await
}

pub(crate) async fn finish_tag<B: Backend>(
    state: &AppState<B>,
    ui: &UiQuery,
    headers: &HeaderMap,
    outcome: Result<(), WebError>,
) -> Response {
    finish_kind(state, ui, headers, outcome, MutationKind::Tag, None, "").await
}

pub(crate) async fn finish_group<B: Backend>(
    state: &AppState<B>,
    ui: &UiQuery,
    headers: &HeaderMap,
    outcome: Result<(), WebError>,
) -> Response {
    finish_kind(state, ui, headers, outcome, MutationKind::Group, None, "").await
}

pub(crate) async fn finish_view_group<B: Backend>(
    state: &AppState<B>,
    ui: &UiQuery,
    headers: &HeaderMap,
    outcome: Result<(), WebError>,
) -> Response {
    finish_kind(
        state,
        ui,
        headers,
        outcome,
        MutationKind::ViewGroup,
        None,
        "",
    )
    .await
}

pub(crate) async fn finish_tag_create<B: Backend>(
    state: &AppState<B>,
    ui: &UiQuery,
    headers: &HeaderMap,
    group: GroupId,
    draft: &str,
    outcome: Result<(), WebError>,
) -> Response {
    finish_kind(
        state,
        ui,
        headers,
        outcome,
        MutationKind::Tag,
        Some(CreationTarget::Tag(group)),
        draft,
    )
    .await
}

pub(crate) async fn finish_group_create<B: Backend>(
    state: &AppState<B>,
    ui: &UiQuery,
    headers: &HeaderMap,
    draft: &str,
    outcome: Result<(), WebError>,
) -> Response {
    finish_kind(
        state,
        ui,
        headers,
        outcome,
        MutationKind::Group,
        Some(CreationTarget::Group),
        draft,
    )
    .await
}

pub(crate) async fn finish_view_group_create<B: Backend>(
    state: &AppState<B>,
    ui: &UiQuery,
    headers: &HeaderMap,
    draft: &str,
    outcome: Result<(), WebError>,
) -> Response {
    finish_kind(
        state,
        ui,
        headers,
        outcome,
        MutationKind::ViewGroup,
        Some(CreationTarget::ViewGroup),
        draft,
    )
    .await
}

async fn finish_kind<B: Backend>(
    state: &AppState<B>,
    ui: &UiQuery,
    headers: &HeaderMap,
    outcome: Result<(), WebError>,
    kind: MutationKind,
    creation: Option<CreationTarget>,
    draft: &str,
) -> Response {
    let trigger = match kind {
        MutationKind::ViewGroup => VIEWS_CHANGED,
        _ => TAGS_CHANGED,
    };
    finish_sidebar(
        state,
        ui,
        headers,
        Some(trigger),
        |sidebar, _settings, lang| {
            let message = match outcome {
                Ok(()) => {
                    if !htmx::is_htmx(headers) {
                        return FinishAction::Respond(Redirect::to("/").into_response());
                    }
                    return FinishAction::Continue;
                }
                Err(error) => match user_error_message(state, lang, kind, &error) {
                    Some(message) => message,
                    None => return FinishAction::Respond(error.into_response()),
                },
            };
            match creation {
                Some(CreationTarget::ViewGroup) => {
                    sidebar.view_group_create_error = Some(ViewGroupCreationError {
                        message,
                        draft: draft.to_string(),
                    });
                }
                Some(CreationTarget::Group) => {
                    sidebar.group_create_error = Some(CreationError {
                        message,
                        draft: draft.to_string(),
                        group: None,
                    });
                }
                Some(CreationTarget::Tag(group)) => {
                    sidebar.tag_create_error = Some(CreationError {
                        message,
                        draft: draft.to_string(),
                        group: Some(group),
                    });
                }
                None => sidebar.error = Some(message),
            }
            // Une erreur ne déclenche aucun rafraîchissement, sinon la sidebar OOB perd son message.
            FinishAction::WithoutTrigger
        },
    )
    .await
}

/// Aucun texte SQLite ne doit atteindre l'UI : conflit toujours remplacé.
pub(crate) fn user_error_message(
    state: &AppState<impl Backend>,
    lang: &str,
    kind: MutationKind,
    error: &WebError,
) -> Option<String> {
    match error {
        WebError::Conflict(_) => Some(match kind {
            MutationKind::Tag => state.locales.t(lang, "tags.tag_exists"),
            MutationKind::Group => state.locales.t(lang, "tags.group_exists"),
            MutationKind::ViewGroup => state.locales.t(lang, "views.group_exists"),
            MutationKind::Other => state.locales.t(lang, "errors.conflict"),
        }),
        WebError::BadRequest(message) => Some(message.clone()),
        WebError::NotFound(_) => Some(error.to_string()),
        _ => None,
    }
}

pub(crate) async fn no_content(result: Result<(), WebError>) -> Response {
    match result {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => error.into_response(),
    }
}
