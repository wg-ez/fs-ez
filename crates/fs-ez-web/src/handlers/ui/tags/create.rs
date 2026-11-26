//! Création d'un tag dans un groupe, erreurs rendues dans la sidebar.

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use fs_ez_app::{AppError, random_tag_color};

use crate::backend::{Backend, blocking};
use crate::forms::parse_optional_color;
use crate::handlers::ui::chrome::UiQuery;
use crate::handlers::ui::form::{UiForm, first_value};
use crate::handlers::ui::pages::load_lang;
use crate::state::AppState;

use super::{clean_tag_name, finish_tag_create, parse_group_id};

pub async fn create_tag<B: Backend>(
    State(state): State<AppState<B>>,
    Query(ui): Query<UiQuery>,
    headers: HeaderMap,
    UiForm(pairs): UiForm,
) -> Response {
    let lang = match load_lang(&state, &headers, &ui).await {
        Ok(lang) => lang,
        Err(error) => return error.into_response(),
    };
    let raw_name = first_value(&pairs, "name").unwrap_or_default();
    let group = match parse_group_id(first_value(&pairs, "group")) {
        Ok(id) => id,
        Err(error) => return error.into_response(),
    };
    let parsed = clean_tag_name(&state, &lang, raw_name).and_then(|name| {
        let color = match parse_optional_color(first_value(&pairs, "color"))? {
            Some(color) => color,
            None => random_tag_color()?,
        };
        Ok((name, color))
    });
    let (name, color) = match parsed {
        Ok(values) => values,
        Err(error) => {
            return finish_tag_create(&state, &ui, &headers, group, raw_name, Err(error)).await;
        }
    };
    let result = blocking(state.backend.app(), move |app| {
        if app.store.get_tag_by_name(&name)?.is_some() {
            return Err(AppError::Conflict(format!("tag {name}")));
        }
        let tag = app.create_tag(group, &name, color)?;
        app.prepend_tag(group, &tag.name)?;
        Ok(())
    })
    .await;
    finish_tag_create(&state, &ui, &headers, group, raw_name, result).await
}
