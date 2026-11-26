//! Mutations d'un tag existant : placement, renommage, couleur et suppression.

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use fs_ez_app::{App, AppError};
use fs_ez_core::TagId;

use crate::backend::{Backend, blocking};
use crate::error::WebError;
use crate::forms::{parse_positive_id, parse_required_color};
use crate::handlers::ui::chrome::UiQuery;
use crate::handlers::ui::form::{UiForm, first_value};
use crate::handlers::ui::pages::load_lang;
use crate::state::AppState;

use super::{clean_tag_name, finish, finish_tag, no_content, parse_group_id};

pub async fn place_tag<B: Backend>(
    State(state): State<AppState<B>>,
    UiForm(pairs): UiForm,
) -> Response {
    let group = match parse_group_id(first_value(&pairs, "group")) {
        Ok(id) => id,
        Err(error) => return error.into_response(),
    };
    let tag = first_value(&pairs, "tag").unwrap_or_default().to_string();
    if tag.trim().is_empty() {
        return WebError::BadRequest("tag is required".to_string()).into_response();
    }
    let before = match first_value(&pairs, "before").map(str::trim) {
        Some(raw) if !raw.is_empty() => match parse_positive_id(raw) {
            Some(id) => Some(TagId(id)),
            None => {
                return WebError::BadRequest("invalid tag id".to_string()).into_response();
            }
        },
        _ => None,
    };
    let result = blocking(state.backend.app(), move |app| {
        app.place_tag(group, &tag, before)
    })
    .await;
    no_content(result).await
}

pub async fn rename_tag<B: Backend>(
    State(state): State<AppState<B>>,
    Query(ui): Query<UiQuery>,
    Path(id): Path<TagId>,
    headers: HeaderMap,
    UiForm(pairs): UiForm,
) -> Response {
    let lang = match load_lang(&state, &headers, &ui).await {
        Ok(lang) => lang,
        Err(error) => return error.into_response(),
    };
    let raw_name = first_value(&pairs, "name").unwrap_or_default();
    let name = match clean_tag_name(&state, &lang, raw_name) {
        Ok(name) => name,
        Err(error) => return finish_tag(&state, &ui, &headers, Err(error)).await,
    };
    let result = blocking(state.backend.app(), move |app| {
        let tag = tag_name(app, id)?;
        app.rename_tag(&tag, &name)?;
        Ok(())
    })
    .await;
    finish_tag(&state, &ui, &headers, result).await
}

pub async fn set_tag_color<B: Backend>(
    State(state): State<AppState<B>>,
    Query(ui): Query<UiQuery>,
    Path(id): Path<TagId>,
    headers: HeaderMap,
    UiForm(pairs): UiForm,
) -> Response {
    let color = match parse_required_color(first_value(&pairs, "color")) {
        Ok(color) => color,
        Err(error) => return finish(&state, &ui, &headers, Err(error)).await,
    };
    let result = blocking(state.backend.app(), move |app| {
        let tag = tag_name(app, id)?;
        app.set_tag_color(&tag, color, None)?;
        Ok(())
    })
    .await;
    finish(&state, &ui, &headers, result).await
}

pub async fn delete_tag<B: Backend>(
    State(state): State<AppState<B>>,
    Query(ui): Query<UiQuery>,
    Path(id): Path<TagId>,
    headers: HeaderMap,
) -> Response {
    let result = blocking(state.backend.app(), move |app| {
        let tag = tag_name(app, id)?;
        app.delete_tag(&tag)?;
        Ok(())
    })
    .await;
    finish(&state, &ui, &headers, result).await
}

/// Tag existant ou `NotFound` nommé, lookup partagé des mutations par id.
fn tag_name(app: &App, id: TagId) -> Result<String, AppError> {
    app.store
        .get_tag(id)?
        .map(|tag| tag.name)
        .ok_or_else(|| AppError::NotFound(format!("tag {id}")))
}
