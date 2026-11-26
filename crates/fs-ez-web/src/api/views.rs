//! API JSON des vues sauvegardées et de leur exécution paginée.

use axum::extract::{Path, Query, State};
use axum::response::Response;
use fs_ez_app::{ViewInput, coherent_filter, resolve_view_filter};
use fs_ez_core::{FilterExpr, ViewGroupId, ViewId};
use serde::Deserialize;

use crate::backend::{Backend, blocking};
use crate::error::WebError;
use crate::state::AppState;

use super::dto::{FileDto, PageDto, ViewDto};
use super::errors::{
    ApiJson, MAX_POSITION, delete_ok, deserialize_double_option, json_created, json_ok, parse_id,
    parse_limit, parse_offset, parse_optional_color, parse_patch_color, parse_sort,
};
use crate::forms::parse_view_icon;

#[derive(Debug, Deserialize)]
pub struct ViewCreate {
    pub name: String,
    #[serde(default, alias = "filter")]
    pub filter_json: Option<FilterExpr>,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub position: Option<i64>,
    #[serde(default)]
    pub sort: Option<String>,
    #[serde(default)]
    pub group: Option<ViewGroupId>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ViewPatch {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, alias = "filter")]
    pub filter_json: Option<FilterExpr>,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub icon: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    pub color: Option<Option<String>>,
    #[serde(default)]
    pub position: Option<i64>,
    #[serde(default)]
    pub sort: Option<String>,
    #[serde(default)]
    pub group: Option<ViewGroupId>,
}

pub async fn list<B: Backend>(State(state): State<AppState<B>>) -> Result<Response, WebError> {
    let views = blocking(state.backend.app(), |app| {
        Ok(app
            .views_with_counts()?
            .into_iter()
            .map(|(view, count)| ViewDto::from_view(view, count))
            .collect::<Vec<_>>())
    })
    .await?;
    Ok(json_ok(ViewsEnvelope { views }))
}

#[derive(Debug, serde::Serialize)]
struct ViewsEnvelope {
    views: Vec<ViewDto>,
}

pub async fn create<B: Backend>(
    State(state): State<AppState<B>>,
    ApiJson(body): ApiJson<ViewCreate>,
) -> Result<Response, WebError> {
    if body
        .position
        .is_some_and(|position| !(0..=MAX_POSITION).contains(&position))
    {
        return Err(WebError::BadRequest("invalid position".to_string()));
    }
    let filter = resolve_filter(body.filter_json, body.query.as_deref())?
        .ok_or_else(|| WebError::BadRequest("filter_json or query required".to_string()))?;
    let input = ViewInput {
        name: body.name,
        filter: Some(filter),
        icon: parse_view_icon(body.icon)?,
        color: parse_optional_color(body.color.as_deref())?,
        position: body.position,
        sort: parse_sort(body.sort.as_deref())?.unwrap_or_default(),
    };
    let (view, count) = blocking(state.backend.app(), move |app| {
        let view = app.create_view_in_group(input, body.group)?;
        app.view_with_count(view.id)
    })
    .await?;
    Ok(json_created(ViewDto::from_view(view, count)))
}

pub async fn get<B: Backend>(
    State(state): State<AppState<B>>,
    Path(id): Path<String>,
) -> Result<Response, WebError> {
    let id = ViewId(parse_id(&id)?);
    let (view, count) = blocking(state.backend.app(), move |app| app.view_with_count(id)).await?;
    Ok(json_ok(ViewDto::from_view(view, count)))
}

pub async fn patch<B: Backend>(
    State(state): State<AppState<B>>,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<ViewPatch>,
) -> Result<Response, WebError> {
    let id = ViewId(parse_id(&id)?);
    if body
        .position
        .is_some_and(|position| !(0..=MAX_POSITION).contains(&position))
    {
        return Err(WebError::BadRequest("invalid position".to_string()));
    }
    let has_icon = body.icon.is_some();
    let icon = body.icon.map(parse_view_icon).transpose()?.flatten();
    let color = parse_patch_color(body.color.as_ref())?;
    let sort = match body.sort.as_deref() {
        Some(raw) => Some(parse_sort(Some(raw))?.unwrap_or_default()),
        None => None,
    };
    let filter = resolve_filter(body.filter_json, body.query.as_deref())?;
    let (view, count) = blocking(state.backend.app(), move |app| {
        let existing = app.get_view(id)?;
        let name = body.name.unwrap_or_else(|| existing.name.clone());
        let input = ViewInput {
            name,
            filter: filter.or(Some(existing.filter.clone())),
            icon: if has_icon { icon } else { existing.icon },
            color: match color {
                Some(value) => value,
                None => existing.color,
            },
            position: body.position,
            sort: sort.unwrap_or(existing.sort),
        };
        let view = app.update_view(id, input)?;
        if let Some(group) = body.group {
            app.assign_view(group, view.id)?;
        }
        app.view_with_count(view.id)
    })
    .await?;
    Ok(json_ok(ViewDto::from_view(view, count)))
}

pub async fn delete<B: Backend>(
    State(state): State<AppState<B>>,
    Path(id): Path<String>,
) -> Result<Response, WebError> {
    let id = ViewId(parse_id(&id)?);
    delete_ok(&state, "view not found", move |app| app.delete_view(id)).await
}

#[derive(Debug, Default, Deserialize)]
pub struct PageQuery {
    #[serde(default)]
    pub offset: Option<String>,
    #[serde(default)]
    pub limit: Option<String>,
}

pub async fn files<B: Backend>(
    State(state): State<AppState<B>>,
    Path(id): Path<String>,
    Query(query): Query<PageQuery>,
) -> Result<Response, WebError> {
    let id = ViewId(parse_id(&id)?);
    let limit = parse_limit(query.limit.as_deref())?;
    let offset = parse_offset(query.offset.as_deref())?;
    let page = blocking(state.backend.app(), move |app| {
        app.run_view(id, offset, limit)
    })
    .await?;
    Ok(json_ok(PageDto::from_page(page, FileDto::from_file)))
}

fn resolve_filter(
    filter_json: Option<FilterExpr>,
    query: Option<&str>,
) -> Result<Option<FilterExpr>, WebError> {
    match filter_json {
        Some(filter) => Ok(Some(coherent_filter(filter, query)?)),
        None => Ok(resolve_view_filter(None, query)?),
    }
}
