//! Paramètres d'URL de la liste de fichiers, validés avant tout accès au store.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use fs_ez_app::{FileListMode, FileListRequest};
use fs_ez_core::{
    FilterExpr, Sort, SortDir, ViewId, VirtualFolderId, VirtualFolderScope, has_unsafe_text,
    normalize_tag_name,
};

use crate::error::WebError;
use crate::forms::parse_positive_id;

#[cfg(test)]
use fs_ez_core::SortField;

/// Nombre maximal de tags simultanés dans un filtre d'URL.
const MAX_TAG_FILTERS: usize = 32;

/// Forme stable de la racine de l'explorateur virtuel dans les URLs.
const VIRTUAL_FOLDER_ROOT: &str = "root";

#[derive(Debug, Clone, Default)]
pub struct FilesParams {
    pub q: Option<String>,
    pub filter: Option<String>,
    pub filter_json: Option<String>,
    pub tags: Vec<String>,
    pub exclude_tags: Vec<String>,
    pub sort: Option<String>,
    pub offset: Option<String>,
    pub limit: Option<String>,
    pub view: Option<ViewId>,
    pub virtual_folder: Option<String>,
}

/// Valeur vide = vidage différé du lot, sinon ajout dans l'ordre d'arrivée.
fn push_tag_value(value: String, tags: &mut Vec<String>, clear: &mut bool) {
    if value.trim().is_empty() {
        *clear = true;
    } else {
        tags.push(value);
    }
}

impl FilesParams {
    /// Extrait brut : `tag` et `not_tag` sont répétables, une valeur vide annule les tags correspondants.
    pub fn from_query(raw: Option<&str>) -> Result<Self, WebError> {
        let mut params = Self::default();
        let mut clear_tags = false;
        let mut clear_exclude = false;
        for (key, value) in form_urlencoded::parse(raw.unwrap_or_default().as_bytes()) {
            match key.as_ref() {
                "q" => params.q = Some(value.into_owned()),
                "filter" => params.filter = Some(value.into_owned()),
                "filter_json" => params.filter_json = Some(value.into_owned()),
                "tag" => push_tag_value(value.into_owned(), &mut params.tags, &mut clear_tags),
                "not_tag" => push_tag_value(
                    value.into_owned(),
                    &mut params.exclude_tags,
                    &mut clear_exclude,
                ),
                "sort" => params.sort = Some(value.into_owned()),
                "offset" => params.offset = Some(value.into_owned()),
                "limit" => params.limit = Some(value.into_owned()),
                "view" => {
                    let id = value
                        .parse::<i64>()
                        .map_err(|_| WebError::BadRequest("invalid view id".to_string()))?;
                    params.view = Some(ViewId(id));
                }
                "vf" => params.virtual_folder = Some(value.into_owned()),
                _ => {}
            }
        }
        if clear_tags {
            params.tags.clear();
        }
        if clear_exclude {
            params.exclude_tags.clear();
        }
        Ok(params)
    }

    /// Paramètres d'une sélection globale : chemin de liste + query de première page.
    pub fn from_scope_url(raw: &str) -> Result<Self, WebError> {
        let (path, query) = raw.split_once('?').unwrap_or((raw, ""));
        let mut params = Self::from_query(Some(query))?;
        if let Some(id) = path.strip_prefix("/views/") {
            let id = id
                .parse::<i64>()
                .map_err(|_| WebError::BadRequest("invalid scope view".to_string()))?;
            params.view = Some(ViewId(id));
        } else if path == "/virtual-folders" {
            params.set_virtual_folder_root();
        } else if let Some(id) = path.strip_prefix("/virtual-folders/") {
            let id = parse_positive_id(id)
                .ok_or_else(|| WebError::BadRequest("invalid scope virtual folder".to_string()))?;
            params.set_virtual_folder(VirtualFolderId(id));
        } else if path != "/" {
            return Err(WebError::BadRequest("invalid scope url".to_string()));
        }
        Ok(params)
    }

    /// Portée racine de l'explorateur virtuel, jeton interne à la crate.
    pub fn set_virtual_folder_root(&mut self) {
        self.virtual_folder = Some(VIRTUAL_FOLDER_ROOT.to_string());
    }

    /// Portée d'un dossier virtuel identifié, jeton stable dans les URLs.
    pub fn set_virtual_folder(&mut self, id: VirtualFolderId) {
        self.virtual_folder = Some(id.to_string());
    }
}

/// Extrait de requête UI : gère les paramètres `tag` répétés.
pub struct FilesQuery(pub FilesParams);

impl<S> FromRequestParts<S> for FilesQuery
where
    S: Send + Sync,
{
    type Rejection = WebError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        FilesParams::from_query(parts.uri.query()).map(FilesQuery)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFilter {
    Untagged,
    Recent,
    RecentFiles,
}

impl FileFilter {
    pub fn token(&self) -> &'static str {
        match self {
            Self::Untagged => "untagged",
            Self::Recent => "recent",
            Self::RecentFiles => "recent-files",
        }
    }
}

/// Valeur non vide après trim, `None` sinon : politique commune aux paramètres optionnels.
pub fn trimmed_non_empty(raw: Option<&str>) -> Option<&str> {
    raw.map(str::trim).filter(|value| !value.is_empty())
}

/// Texte de recherche borné, vide ramené à `None`.
pub fn parse_search_text(raw: Option<&str>) -> Result<Option<String>, WebError> {
    let Some(text) = trimmed_non_empty(raw) else {
        return Ok(None);
    };
    if has_unsafe_text(text) {
        return Err(WebError::BadRequest("invalid search text".to_string()));
    }
    if text.chars().count() > fs_ez_store::MAX_TEXT_LEN {
        return Err(WebError::BadRequest(format!(
            "search text exceeds {} characters",
            fs_ez_store::MAX_TEXT_LEN
        )));
    }
    Ok(Some(text.to_string()))
}

/// Arbre de filtres sérialisé, absent ou vide ramené à `None`.
pub fn parse_filter_json(raw: Option<&str>) -> Result<Option<FilterExpr>, WebError> {
    let Some(raw) = trimmed_non_empty(raw) else {
        return Ok(None);
    };
    let filter: FilterExpr = serde_json::from_str(raw)
        .map_err(|error| WebError::BadRequest(format!("invalid filter json: {error}")))?;
    filter
        .validate()
        .map_err(|error| WebError::BadRequest(error.to_string()))?;
    Ok(Some(filter))
}

/// Filtre `untagged`/`recent`/`recent-files`, toute autre valeur refusée.
pub fn parse_file_filter(raw: Option<&str>) -> Result<Option<FileFilter>, WebError> {
    match raw {
        None => Ok(None),
        Some("untagged") => Ok(Some(FileFilter::Untagged)),
        Some("recent") => Ok(Some(FileFilter::Recent)),
        Some("recent-files") => Ok(Some(FileFilter::RecentFiles)),
        Some(other) => Err(WebError::BadRequest(format!("unknown filter {other:?}"))),
    }
}

/// Nom affichable + clé normalisée, erreur commune UI/API.
fn normalize_tag(raw: &str) -> Result<(String, String), WebError> {
    normalize_tag_name(raw).map_err(|_| WebError::BadRequest("invalid tag name".to_string()))
}

/// Nom de tag normalisé, vide ramené à `None`.
pub fn parse_tag_filter(raw: Option<&str>) -> Result<Option<String>, WebError> {
    match trimmed_non_empty(raw) {
        None => Ok(None),
        Some(raw) => normalize_tag(raw).map(|(display, _)| Some(display)),
    }
}

/// Tags normalisés, doublons retirés dans l'ordre d'arrivée.
pub fn parse_tag_filters(raws: &[String]) -> Result<Vec<String>, WebError> {
    let mut tags: Vec<String> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for raw in raws {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        let (display, normalized) = normalize_tag(raw)?;
        if !seen.contains(&normalized) {
            seen.push(normalized);
            tags.push(display);
        }
    }
    if tags.len() > MAX_TAG_FILTERS {
        return Err(WebError::BadRequest(format!(
            "too many tag filters (max {MAX_TAG_FILTERS})"
        )));
    }
    Ok(tags)
}

fn parse_usize(raw: &str, field: &str) -> Result<usize, WebError> {
    raw.parse::<usize>()
        .map_err(|_| WebError::BadRequest(format!("invalid {field}")))
}

/// Nombre brut sans trim ni défaut, politique laissée aux appelants.
pub fn parse_offset_number(raw: &str) -> Result<usize, WebError> {
    parse_usize(raw, "offset")
}

/// Nombre brut sans trim, défaut ni borne haute, politique laissée aux appelants.
pub fn parse_limit_number(raw: &str) -> Result<usize, WebError> {
    parse_usize(raw, "limit")
}

/// Option de nombre : `""` = absent, aucune trim, le parseur garde son message.
fn parse_opt<T>(
    raw: Option<&str>,
    parse: impl Fn(&str) -> Result<T, WebError>,
) -> Result<Option<T>, WebError> {
    raw.filter(|value| !value.is_empty()).map(parse).transpose()
}

#[derive(Debug, Clone)]
pub struct ValidatedFiles {
    pub q: Option<String>,
    pub filter: Option<FileFilter>,
    pub filter_expr: Option<FilterExpr>,
    pub tags: Vec<String>,
    pub exclude_tags: Vec<String>,
    pub sort: Option<Sort>,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
    pub view: Option<ViewId>,
    pub virtual_folder: Option<VirtualFolderScope>,
}

impl FilesParams {
    /// Toute valeur inconnue est un 400 : jamais de repli silencieux sur un filtre.
    pub fn validate(&self) -> Result<ValidatedFiles, WebError> {
        let q = parse_search_text(self.q.as_deref())?;

        let filter = parse_file_filter(self.filter.as_deref())?;

        let filter_expr = parse_filter_json(self.filter_json.as_deref())?;

        let tags = parse_tag_filters(&self.tags)?;

        let exclude_tags = parse_tag_filters(&self.exclude_tags)?;

        if filter == Some(FileFilter::Untagged) {
            if !tags.is_empty() {
                return Err(WebError::BadRequest(
                    "untagged cannot be combined with a tag".to_string(),
                ));
            }
            if !exclude_tags.is_empty() {
                return Err(WebError::BadRequest(
                    "untagged cannot be combined with an excluded tag".to_string(),
                ));
            }
            if filter_expr.is_some() {
                return Err(WebError::BadRequest(
                    "untagged cannot be combined with a filter".to_string(),
                ));
            }
        }

        let sort = self.sort.as_deref().map(parse_sort_arg).transpose()?;

        let offset = parse_opt(self.offset.as_deref(), parse_offset_number)?;
        let limit = parse_opt(self.limit.as_deref(), parse_limit_number)?;

        let virtual_folder = match self.virtual_folder.as_deref() {
            Some("") | Some(VIRTUAL_FOLDER_ROOT) => Some(VirtualFolderScope::Root),
            Some(raw) => {
                let id = parse_positive_id(raw)
                    .ok_or_else(|| WebError::BadRequest("invalid virtual folder id".to_string()))?;
                Some(VirtualFolderScope::Folder(VirtualFolderId(id)))
            }
            None => None,
        };

        Ok(ValidatedFiles {
            q,
            filter,
            filter_expr,
            tags,
            exclude_tags,
            sort,
            offset,
            limit,
            view: self.view,
            virtual_folder,
        })
    }
}

/// Forme `champ` ou `champ:direction`, direction ascendante par défaut comme l'API et la CLI.
pub fn parse_sort(raw: &str) -> Option<Sort> {
    Sort::decode_with_default_dir(raw, SortDir::Asc)
}

/// Tri requis : toute valeur non décodable devient un 400 au texte commun UI/API.
pub fn parse_sort_arg(raw: &str) -> Result<Sort, WebError> {
    parse_sort(raw).ok_or_else(|| WebError::BadRequest(format!("invalid sort {raw:?}")))
}

pub fn sort_token(sort: Sort) -> String {
    sort.encode()
}

/// Les paramètres UI et API convergent ici : la résolution métier reste dans `fs-ez-app`.
pub fn build_file_request(params: &ValidatedFiles, limit: usize, total: bool) -> FileListRequest {
    let filter = FilterExpr::and_all(
        params
            .filter_expr
            .iter()
            .cloned()
            .chain(exclude_filter(&params.exclude_tags)),
    );
    FileListRequest {
        view: params.view,
        filter,
        tags: params.tags.clone(),
        text: params.q.clone(),
        mode: match params.filter {
            Some(FileFilter::Untagged) => FileListMode::Untagged,
            Some(FileFilter::Recent) => FileListMode::Recent,
            Some(FileFilter::RecentFiles) => FileListMode::RecentFiles,
            None => FileListMode::All,
        },
        sort: params.sort,
        virtual_folder: params.virtual_folder,
        offset: params.offset.unwrap_or(0),
        limit,
        total,
        ..FileListRequest::new(limit)
    }
}

/// Blacklist : exclut les fichiers portant au moins un des tags nommés.
fn exclude_filter(tags: &[String]) -> Option<FilterExpr> {
    FilterExpr::and_all(tags.iter().map(|tag| FilterExpr::Tag {
        tag: tag.clone(),
        negated: true,
    }))
}

impl ValidatedFiles {
    /// Forme stable du dossier virtuel dans les URLs, `root` pour la racine.
    pub fn virtual_folder_token(&self) -> Option<String> {
        self.virtual_folder.map(|folder| match folder {
            VirtualFolderScope::Folder(id) => id.to_string(),
            VirtualFolderScope::Root => VIRTUAL_FOLDER_ROOT.to_string(),
        })
    }
}

/// Forme JSON compacte conservée dans l'URL, `None` si la sérialisation échoue.
pub fn filter_json_token(filter: &FilterExpr) -> Option<String> {
    serde_json::to_string(filter).ok()
}

#[cfg(test)]
mod tests;
