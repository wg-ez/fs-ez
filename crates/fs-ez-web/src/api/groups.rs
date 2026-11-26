//! API JSON des groupes de tags.

use axum::extract::{Path, State};
use axum::response::Response;
use fs_ez_core::GroupId;
use serde::Deserialize;

use crate::backend::{Backend, blocking};
use crate::error::WebError;
use crate::state::AppState;

use super::dto::GroupDto;
use super::errors::{
    ApiJson, check_assign_action, delete_ok, deserialize_double_option, json_created, json_ok,
    parse_id, parse_optional_color, parse_patch_color,
};

#[derive(Debug, Deserialize)]
pub struct CreateGroup {
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct PatchGroup {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub color: Option<Option<String>>,
    #[serde(default)]
    pub collapsed: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct GroupTagsBody {
    pub tag: String,
    pub action: String,
}

pub async fn list<B: Backend>(State(state): State<AppState<B>>) -> Result<Response, WebError> {
    let groups = blocking(state.backend.app(), |app| {
        let groups = app.groups_with_tags()?;
        Ok(groups
            .into_iter()
            .map(|(group, tags)| GroupDto::from_group(group, tags))
            .collect())
    })
    .await?;
    Ok(json_ok(GroupsEnvelope { groups }))
}

#[derive(Debug, serde::Serialize)]
struct GroupsEnvelope {
    groups: Vec<GroupDto>,
}

pub async fn create<B: Backend>(
    State(state): State<AppState<B>>,
    ApiJson(body): ApiJson<CreateGroup>,
) -> Result<Response, WebError> {
    let color = parse_optional_color(body.color.as_deref())?;
    let name = body.name;
    let group = blocking(state.backend.app(), move |app| {
        app.create_group(&name, color)
    })
    .await?;
    Ok(json_created(GroupDto::from_group(group, Vec::new())))
}

pub async fn patch<B: Backend>(
    State(state): State<AppState<B>>,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<PatchGroup>,
) -> Result<Response, WebError> {
    let id = GroupId(parse_id(&id)?);
    let color = parse_patch_color(body.color.as_ref())?;
    let group = blocking(state.backend.app(), move |app| {
        if let Some(name) = body.name.as_deref() {
            app.rename_group(id, name)?;
        }
        if let Some(color) = color {
            app.set_group_color(id, color)?;
        }
        if let Some(collapsed) = body.collapsed {
            app.set_group_collapsed(id, collapsed)?;
        }
        let (group, tags) = app.group_with_tags(id)?;
        Ok(GroupDto::from_group(group, tags))
    })
    .await?;
    Ok(json_ok(group))
}

pub async fn delete<B: Backend>(
    State(state): State<AppState<B>>,
    Path(id): Path<String>,
) -> Result<Response, WebError> {
    let id = GroupId(parse_id(&id)?);
    delete_ok(&state, "group not found", move |app| app.delete_group(id)).await
}

pub async fn mutate_tags<B: Backend>(
    State(state): State<AppState<B>>,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<GroupTagsBody>,
) -> Result<Response, WebError> {
    let id = GroupId(parse_id(&id)?);
    check_assign_action(&body.action, "tag")?;
    let tag = body.tag;
    let group = blocking(state.backend.app(), move |app| {
        app.assign_tag(id, &tag)?;
        let (group, tags) = app.group_with_tags(id)?;
        Ok(GroupDto::from_group(group, tags))
    })
    .await?;
    Ok(json_ok(group))
}
