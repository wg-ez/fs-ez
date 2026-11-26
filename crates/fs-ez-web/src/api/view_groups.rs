//! API JSON des groupes de vues.

use axum::extract::{Path, State};
use axum::response::Response;
use fs_ez_app::{App, AppError};
use fs_ez_core::{SavedView, ViewGroupId, ViewId};
use serde::Deserialize;

use crate::backend::{Backend, blocking};
use crate::error::WebError;
use crate::state::AppState;

use super::dto::{ViewDto, ViewGroupDto};
use super::errors::{
    ApiJson, MAX_POSITION, check_assign_action, delete_ok, deserialize_double_option, json_created,
    json_ok, parse_id, parse_optional_color, parse_patch_color,
};

#[derive(Debug, Deserialize)]
pub struct CreateViewGroup {
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct PatchViewGroup {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub color: Option<Option<String>>,
    #[serde(default)]
    pub collapsed: Option<bool>,
    #[serde(default)]
    pub position: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct GroupViewsBody {
    pub view: ViewId,
    pub action: String,
}

/// DTO des vues avec leur compte, ordre conservé.
fn view_dtos(app: &App, views: Vec<SavedView>) -> Result<Vec<ViewDto>, AppError> {
    views
        .into_iter()
        .map(|view| {
            let count = app.count_view(view.id)?;
            Ok(ViewDto::from_view(view, count))
        })
        .collect()
}

pub async fn list<B: Backend>(State(state): State<AppState<B>>) -> Result<Response, WebError> {
    let groups = blocking(state.backend.app(), |app| {
        app.view_groups_with_views()?
            .into_iter()
            .map(|(group, views)| Ok(ViewGroupDto::from_group(group, view_dtos(app, views)?)))
            .collect()
    })
    .await?;
    Ok(json_ok(GroupsEnvelope { groups }))
}

#[derive(Debug, serde::Serialize)]
struct GroupsEnvelope {
    groups: Vec<ViewGroupDto>,
}

pub async fn create<B: Backend>(
    State(state): State<AppState<B>>,
    ApiJson(body): ApiJson<CreateViewGroup>,
) -> Result<Response, WebError> {
    let color = parse_optional_color(body.color.as_deref())?;
    let name = body.name;
    let group = blocking(state.backend.app(), move |app| {
        app.create_view_group(&name, color)
    })
    .await?;
    Ok(json_created(ViewGroupDto::from_group(group, Vec::new())))
}

pub async fn patch<B: Backend>(
    State(state): State<AppState<B>>,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<PatchViewGroup>,
) -> Result<Response, WebError> {
    let id = ViewGroupId(parse_id(&id)?);
    if body
        .position
        .is_some_and(|position| !(0..=MAX_POSITION).contains(&position))
    {
        return Err(WebError::BadRequest("invalid position".to_string()));
    }
    let color = parse_patch_color(body.color.as_ref())?;
    let group = blocking(state.backend.app(), move |app| {
        if let Some(name) = body.name.as_deref() {
            app.rename_view_group(id, name)?;
        }
        if let Some(color) = color {
            app.set_view_group_color(id, color)?;
        }
        if let Some(collapsed) = body.collapsed {
            app.set_view_group_collapsed(id, collapsed)?;
        }
        if let Some(position) = body.position {
            app.store
                .update_view_group(id, None, None, Some(position), None)?;
        }
        let (group, views) = app.view_group_with_views(id)?;
        Ok(ViewGroupDto::from_group(group, view_dtos(app, views)?))
    })
    .await?;
    Ok(json_ok(group))
}

pub async fn delete<B: Backend>(
    State(state): State<AppState<B>>,
    Path(id): Path<String>,
) -> Result<Response, WebError> {
    let id = ViewGroupId(parse_id(&id)?);
    delete_ok(&state, "view group not found", move |app| {
        app.delete_view_group(id)
    })
    .await
}

pub async fn mutate_views<B: Backend>(
    State(state): State<AppState<B>>,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<GroupViewsBody>,
) -> Result<Response, WebError> {
    let id = ViewGroupId(parse_id(&id)?);
    check_assign_action(&body.action, "view")?;
    let view = body.view;
    let group = blocking(state.backend.app(), move |app| {
        app.assign_view(id, view)?;
        let (group, views) = app.view_group_with_views(id)?;
        Ok(ViewGroupDto::from_group(group, view_dtos(app, views)?))
    })
    .await?;
    Ok(json_ok(group))
}
