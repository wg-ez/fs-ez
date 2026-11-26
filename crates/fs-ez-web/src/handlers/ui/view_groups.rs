//! Mutations des groupes de vues : CRUD, affectations et déplacements.

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use fs_ez_core::{ViewGroupId, ViewId};

use crate::backend::{Backend, blocking};
use crate::error::WebError;
use crate::forms::{parse_id_list, parse_optional_color, parse_positive_id};
use crate::handlers::ui::chrome::UiQuery;
use crate::handlers::ui::form::{
    GroupFormError, UiForm, first_value, parse_group_draft, parse_group_name,
};
use crate::handlers::ui::tags::{finish_view_group, finish_view_group_create, no_content};
use crate::state::AppState;

pub async fn create_view_group<B: Backend>(
    State(state): State<AppState<B>>,
    Query(ui): Query<UiQuery>,
    headers: HeaderMap,
    UiForm(pairs): UiForm,
) -> Response {
    let raw_name = first_value(&pairs, "name").unwrap_or_default();
    let (name, color) =
        match parse_group_draft(&state, &ui, &headers, &pairs, "views.group_name_required").await {
            Ok(draft) => draft,
            Err(GroupFormError::Form(error)) => {
                return finish_view_group_create(&state, &ui, &headers, raw_name, Err(error)).await;
            }
            Err(GroupFormError::Raw(response)) => return *response,
        };
    let created = blocking(state.backend.app(), move |app| {
        app.create_view_group(&name, color)
    })
    .await;
    finish_view_group_create(&state, &ui, &headers, raw_name, created.map(|_| ())).await
}

fn parse_view_id(raw: Option<&str>) -> Result<ViewId, WebError> {
    raw.map(str::trim)
        .filter(|raw| !raw.is_empty())
        .and_then(parse_positive_id)
        .map(ViewId)
        .ok_or_else(|| WebError::BadRequest("invalid view id".to_string()))
}

fn optional_view_id(raw: Option<&str>) -> Result<Option<ViewId>, WebError> {
    match raw.map(str::trim).filter(|raw| !raw.is_empty()) {
        Some(_) => parse_view_id(raw).map(Some),
        None => Ok(None),
    }
}

pub async fn reorder_view_groups<B: Backend>(
    State(state): State<AppState<B>>,
    UiForm(pairs): UiForm,
) -> Response {
    let ids = match parse_id_list::<ViewGroupId>(first_value(&pairs, "ids")) {
        Ok(ids) => ids,
        Err(error) => return error.into_response(),
    };
    let result = blocking(state.backend.app(), move |app| {
        app.reorder_view_groups(&ids)
    })
    .await;
    no_content(result).await
}

pub async fn rename_view_group<B: Backend>(
    State(state): State<AppState<B>>,
    Query(ui): Query<UiQuery>,
    Path(id): Path<ViewGroupId>,
    headers: HeaderMap,
    UiForm(pairs): UiForm,
) -> Response {
    let raw_name = first_value(&pairs, "name").unwrap_or_default();
    let name = match parse_group_name(&state, &ui, &headers, raw_name, "views.group_name_required")
        .await
    {
        Ok(name) => name,
        Err(GroupFormError::Form(error)) => {
            return finish_view_group(&state, &ui, &headers, Err(error)).await;
        }
        Err(GroupFormError::Raw(response)) => return *response,
    };
    let result = blocking(state.backend.app(), move |app| {
        app.rename_view_group(id, &name).map(|_| ())
    })
    .await;
    finish_view_group(&state, &ui, &headers, result).await
}

pub async fn set_view_group_color<B: Backend>(
    State(state): State<AppState<B>>,
    Query(ui): Query<UiQuery>,
    Path(id): Path<ViewGroupId>,
    headers: HeaderMap,
    UiForm(pairs): UiForm,
) -> Response {
    let color = match parse_optional_color(first_value(&pairs, "color")) {
        Ok(color) => color,
        Err(error) => return finish_view_group(&state, &ui, &headers, Err(error)).await,
    };
    let result = blocking(state.backend.app(), move |app| {
        app.set_view_group_color(id, color).map(|_| ())
    })
    .await;
    finish_view_group(&state, &ui, &headers, result).await
}

pub async fn delete_view_group<B: Backend>(
    State(state): State<AppState<B>>,
    Query(ui): Query<UiQuery>,
    Path(id): Path<ViewGroupId>,
    headers: HeaderMap,
) -> Response {
    let result = blocking(state.backend.app(), move |app| {
        app.delete_view_group(id).map(|_| ())
    })
    .await;
    finish_view_group(&state, &ui, &headers, result).await
}

pub async fn place_view<B: Backend>(
    State(state): State<AppState<B>>,
    Query(ui): Query<UiQuery>,
    headers: HeaderMap,
    UiForm(pairs): UiForm,
) -> Response {
    let group = match parse_id_list::<ViewGroupId>(first_value(&pairs, "group")) {
        Ok(ids) => match ids.into_iter().next() {
            Some(group) => group,
            None => return WebError::BadRequest("group required".to_string()).into_response(),
        },
        Err(error) => return error.into_response(),
    };
    let view = match parse_view_id(first_value(&pairs, "view")) {
        Ok(view) => view,
        Err(error) => return error.into_response(),
    };
    let before = match optional_view_id(first_value(&pairs, "before")) {
        Ok(before) => before,
        Err(error) => return error.into_response(),
    };
    let result = blocking(state.backend.app(), move |app| {
        app.place_view(group, view, before).map(|_| ())
    })
    .await;
    finish_view_group(&state, &ui, &headers, result).await
}
