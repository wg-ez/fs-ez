//! API JSON des fichiers : liste filtrée, détail et mutations de tags.

use axum::extract::{Path, Query, State};
use axum::response::Response;
use fs_ez_app::{FileListMode, FileListRequest, parse_tag_names};
use fs_ez_core::{FileId, FilterExpr, Sort};
use serde::Deserialize;

use crate::backend::{Backend, blocking};
use crate::error::WebError;
use crate::query_params::{
    FileFilter, parse_file_filter, parse_search_text, parse_tag_filter, trimmed_non_empty,
};
use crate::state::AppState;

use super::dto::{FileDto, PageDto};
use super::errors::{ApiJson, json_ok, parse_id, parse_limit, parse_offset, parse_sort};
use super::tag_action::{TagAction, TagsBody};

#[derive(Debug, Default, Deserialize)]
pub struct FilesQuery {
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub filter: Option<String>,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub filter_json: Option<String>,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub sort: Option<String>,
    #[serde(default)]
    pub offset: Option<String>,
    #[serde(default)]
    pub limit: Option<String>,
}

#[derive(Debug)]
struct ListParams {
    text: Option<String>,
    tag: Option<String>,
    filter: Option<FileFilter>,
    filter_expr: Option<FilterExpr>,
    sort: Option<Sort>,
    offset: usize,
    limit: usize,
}

impl FilesQuery {
    fn validate(&self) -> Result<ListParams, WebError> {
        let text = parse_search_text(self.q.as_deref())?;

        let filter = parse_file_filter(self.filter.as_deref())?;

        let tag = parse_tag_filter(self.tag.as_deref())?;

        let query = trimmed_non_empty(self.query.as_deref());
        if query.is_some() && (filter.is_some() || tag.is_some() || self.filter_json.is_some()) {
            return Err(WebError::BadRequest(
                "query is exclusive with filter, tag and filter_json".to_string(),
            ));
        }
        let filter_expr = match query {
            Some(raw) => Some(fs_ez_app::parse_filter_query(raw)?),
            None => fs_ez_app::resolve_view_filter(self.filter_json.as_deref(), None)?,
        };
        if filter.is_some_and(|filter| matches!(filter, FileFilter::Untagged))
            && (tag.is_some() || filter_expr.is_some())
        {
            return Err(WebError::BadRequest(
                "untagged cannot be combined with a filter".to_string(),
            ));
        }

        Ok(ListParams {
            text,
            tag,
            filter,
            filter_expr,
            sort: parse_sort(self.sort.as_deref())?,
            offset: parse_offset(self.offset.as_deref())?,
            limit: parse_limit(self.limit.as_deref())?,
        })
    }
}

pub async fn list<B: Backend>(
    State(state): State<AppState<B>>,
    Query(query): Query<FilesQuery>,
) -> Result<Response, WebError> {
    let params = query.validate()?;
    let page = blocking(state.backend.app(), move |app| {
        let request = FileListRequest {
            filter: params.filter_expr,
            tags: params.tag.into_iter().collect(),
            text: params.text,
            mode: match params.filter {
                Some(FileFilter::Untagged) => FileListMode::Untagged,
                Some(FileFilter::Recent) => FileListMode::Recent,
                Some(FileFilter::RecentFiles) => FileListMode::RecentFiles,
                None => FileListMode::All,
            },
            sort: params.sort,
            offset: params.offset,
            limit: params.limit,
            ..FileListRequest::new(params.limit)
        };
        app.list_files_page(&request)
    })
    .await?;
    let page = page.into_page();
    Ok(json_ok(PageDto::from_page(page, FileDto::from_file)))
}

pub async fn get<B: Backend>(
    State(state): State<AppState<B>>,
    Path(id): Path<String>,
) -> Result<Response, WebError> {
    let id = FileId(parse_id(&id)?);
    let file = blocking(state.backend.app(), move |app| app.file_with_tags(id)).await?;
    Ok(json_ok(FileDto::from_file(file)))
}

pub async fn mutate_tags<B: Backend>(
    State(state): State<AppState<B>>,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<TagsBody>,
) -> Result<Response, WebError> {
    let id = FileId(parse_id(&id)?);
    let action = TagAction::parse(&body.action)?;
    if body.tags.is_empty() {
        return Err(WebError::BadRequest("tags must not be empty".to_string()));
    }
    let tags = parse_tag_names(&body.tags)?;
    let file = blocking(state.backend.app(), move |app| {
        app.file_with_tags(id)?;
        action.apply(app, id, &tags)?;
        app.file_with_tags(id)
    })
    .await?;
    Ok(json_ok(FileDto::from_file(file)))
}

pub async fn remove_tag<B: Backend>(
    State(state): State<AppState<B>>,
    Path((id, tag)): Path<(String, String)>,
) -> Result<Response, WebError> {
    let id = FileId(parse_id(&id)?);

    let file = blocking(state.backend.app(), move |app| {
        app.file_with_tags(id)?;
        let tag = app.resolve_tag_name(&tag)?;
        app.store.remove_tag(id, tag)?;
        app.file_with_tags(id)
    })
    .await?;
    Ok(json_ok(FileDto::from_file(file)))
}
