//! API JSON des tags.

use axum::extract::{Path, State};
use axum::response::Response;
use fs_ez_app::{AppError, random_tag_color};
use fs_ez_core::{GroupId, normalize_tag_name};
use serde::Deserialize;

use crate::backend::{Backend, blocking};
use crate::error::WebError;
use crate::state::AppState;

use super::dto::TagDto;
use super::errors::{
    ApiJson, delete_ok, deserialize_double_option, json_created, json_ok, parse_optional_color,
};

/// Nom normalisé ou 400 stable, identique pour PATCH et DELETE.
fn normalized_tag_name(raw: &str) -> Result<String, WebError> {
    normalize_tag_name(raw)
        .map(|(name, _)| name)
        .map_err(|_| WebError::BadRequest("invalid tag name".to_string()))
}

#[derive(Debug, Deserialize)]
pub struct CreateTag {
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub group: Option<GroupId>,
}

#[derive(Debug, Deserialize)]
pub struct PatchTag {
    #[serde(default)]
    pub name: Option<String>,
    /// `null` = inchangé, la palette d'un tag est non nulle en base.
    #[serde(default)]
    pub color: Option<String>,
    /// Tri-état `Option<Option<String>>` : absent = inchangé, `null` = effacer, chaîne = valider.
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub color_custom: Option<Option<String>>,
}

pub async fn list<B: Backend>(State(state): State<AppState<B>>) -> Result<Response, WebError> {
    let tags = blocking(state.backend.app(), |app| app.list_tags()).await?;
    let tags: Vec<TagDto> = tags.into_iter().map(TagDto::from_count).collect();
    Ok(json_ok(TagsEnvelope { tags }))
}

#[derive(Debug, serde::Serialize)]
struct TagsEnvelope {
    tags: Vec<TagDto>,
}

pub async fn create<B: Backend>(
    State(state): State<AppState<B>>,
    ApiJson(body): ApiJson<CreateTag>,
) -> Result<Response, WebError> {
    let color = parse_optional_color(body.color.as_deref())?.map_or_else(random_tag_color, Ok)?;
    let name = body.name;
    let group = body.group;
    let tag = blocking(state.backend.app(), move |app| {
        let group = group.map_or_else(|| app.fallback_group(), Ok)?;
        app.create_tag(group, &name, color)
    })
    .await?;
    Ok(json_created(TagDto::from_tag(tag, None)))
}

pub async fn patch<B: Backend>(
    State(state): State<AppState<B>>,
    Path(name): Path<String>,
    ApiJson(body): ApiJson<PatchTag>,
) -> Result<Response, WebError> {
    let has_custom = body.color_custom.is_some();
    if body.name.is_none() && body.color.is_none() && !has_custom {
        return Err(WebError::BadRequest("nothing to update".to_string()));
    }
    let name = normalized_tag_name(&name)?;
    let color = parse_optional_color(body.color.as_deref())?;
    let new_name = body.name;
    let custom = body.color_custom;
    let tag = blocking(state.backend.app(), move |app| {
        let current = app
            .store
            .get_tag_by_name(&name)?
            .ok_or_else(|| AppError::NotFound(format!("tag {name}")))?;
        let renamed = match &new_name {
            Some(new_name) if *new_name != current.name => {
                app.rename_tag(&current.name, new_name)?
            }
            _ => current.clone(),
        };
        if color.is_some() || has_custom {
            // Absent = custom existant, null = effacer, chaîne = valider.
            let custom = custom.unwrap_or_else(|| renamed.color_custom.clone());
            return app.set_tag_color(
                &renamed.name,
                color.unwrap_or(renamed.color),
                custom.as_deref(),
            );
        }
        Ok(renamed)
    })
    .await?;
    Ok(json_ok(TagDto::from_tag(tag, None)))
}

pub async fn delete<B: Backend>(
    State(state): State<AppState<B>>,
    Path(name): Path<String>,
) -> Result<Response, WebError> {
    let name = normalized_tag_name(&name)?;
    delete_ok(&state, "tag not found", move |app| app.delete_tag(&name)).await
}
