//! Mutations des groupes de tags : CRUD, affectations et déplacements.

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use fs_ez_core::GroupId;

use crate::backend::{Backend, blocking};
use crate::forms::{parse_id_list, parse_optional_color};
use crate::handlers::ui::chrome::UiQuery;
use crate::handlers::ui::form::{
    GroupFormError, UiForm, first_value, parse_group_draft, parse_group_name,
};
use crate::handlers::ui::tags::{finish, finish_group, finish_group_create, no_content};
use crate::state::AppState;

pub async fn create_group<B: Backend>(
    State(state): State<AppState<B>>,
    Query(ui): Query<UiQuery>,
    headers: HeaderMap,
    UiForm(pairs): UiForm,
) -> Response {
    let raw_name = first_value(&pairs, "name").unwrap_or_default();
    let (name, color) =
        match parse_group_draft(&state, &ui, &headers, &pairs, "tags.group_name_required").await {
            Ok(draft) => draft,
            Err(GroupFormError::Form(error)) => {
                return finish_group_create(&state, &ui, &headers, raw_name, Err(error)).await;
            }
            Err(GroupFormError::Raw(response)) => return *response,
        };
    let created = blocking(state.backend.app(), move |app| {
        app.create_group(&name, color)
    })
    .await;
    finish_group_create(&state, &ui, &headers, raw_name, created.map(|_| ())).await
}

pub async fn reorder_groups<B: Backend>(
    State(state): State<AppState<B>>,
    UiForm(pairs): UiForm,
) -> Response {
    let ids = match parse_id_list::<GroupId>(first_value(&pairs, "ids")) {
        Ok(ids) => ids,
        Err(error) => return error.into_response(),
    };
    let result = blocking(state.backend.app(), move |app| app.reorder_groups(&ids)).await;
    no_content(result).await
}

pub async fn rename_group<B: Backend>(
    State(state): State<AppState<B>>,
    Query(ui): Query<UiQuery>,
    Path(id): Path<GroupId>,
    headers: HeaderMap,
    UiForm(pairs): UiForm,
) -> Response {
    let raw_name = first_value(&pairs, "name").unwrap_or_default();
    let name =
        match parse_group_name(&state, &ui, &headers, raw_name, "tags.group_name_required").await {
            Ok(name) => name,
            Err(GroupFormError::Form(error)) => {
                return finish_group(&state, &ui, &headers, Err(error)).await;
            }
            Err(GroupFormError::Raw(response)) => return *response,
        };
    let result = blocking(state.backend.app(), move |app| {
        app.rename_group(id, &name).map(|_| ())
    })
    .await;
    finish_group(&state, &ui, &headers, result).await
}

pub async fn set_group_color<B: Backend>(
    State(state): State<AppState<B>>,
    Query(ui): Query<UiQuery>,
    Path(id): Path<GroupId>,
    headers: HeaderMap,
    UiForm(pairs): UiForm,
) -> Response {
    let color = match parse_optional_color(first_value(&pairs, "color")) {
        Ok(color) => color,
        Err(error) => return finish_group(&state, &ui, &headers, Err(error)).await,
    };
    let result = blocking(state.backend.app(), move |app| {
        app.set_group_color(id, color).map(|_| ())
    })
    .await;
    finish_group(&state, &ui, &headers, result).await
}

pub async fn delete_group<B: Backend>(
    State(state): State<AppState<B>>,
    Query(ui): Query<UiQuery>,
    Path(id): Path<GroupId>,
    headers: HeaderMap,
) -> Response {
    let result = blocking(state.backend.app(), move |app| {
        app.delete_group(id).map(|_| ())
    })
    .await;
    finish(&state, &ui, &headers, result).await
}
