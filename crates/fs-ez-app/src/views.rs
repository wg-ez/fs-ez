//! Cas d'usage des vues sauvegardées : CRUD, exécution et comptage.

use fs_ez_core::{
    FileWithTags, FilterExpr, NewSavedView, SavedView, SavedViewPatch, Sort, TagColor, ViewGroupId,
    ViewId, normalize_view_icon, normalize_view_name,
};
use fs_ez_query::pretty;
use fs_ez_store::{Page, validate_page_limit};

use crate::app::App;
use crate::error::AppError;

#[derive(Debug, Clone, PartialEq)]
pub struct ViewInput {
    pub name: String,
    pub filter: Option<FilterExpr>,
    pub icon: Option<String>,
    pub color: Option<TagColor>,
    pub position: Option<i64>,
    pub sort: Sort,
}

fn view_name(raw: &str) -> Result<String, AppError> {
    normalize_view_name(raw).ok_or_else(|| AppError::Invalid(format!("invalid view name {raw:?}")))
}

fn view_icon(raw: Option<String>) -> Result<Option<String>, AppError> {
    match raw {
        Some(raw) => {
            normalize_view_icon(&raw).map_err(|error| AppError::Invalid(error.to_string()))
        }
        None => Ok(None),
    }
}

fn validated_view_fields(
    input: &ViewInput,
) -> Result<(String, FilterExpr, Option<String>), AppError> {
    Ok((
        view_name(&input.name)?,
        required_filter(input.filter.clone())?,
        view_icon(input.icon.clone())?,
    ))
}

/// Parse la forme textuelle d'un filtre, utilisée par l'UI, l'API et la CLI.
pub fn parse_filter_query(raw: &str) -> Result<FilterExpr, AppError> {
    Ok(fs_ez_query::parse(raw)?)
}

/// Parse un filtre JSON, format du constructeur visuel, validé avant usage.
fn parse_filter_json(raw: &str) -> Result<FilterExpr, AppError> {
    let filter: FilterExpr = serde_json::from_str(raw)
        .map_err(|error| AppError::Invalid(format!("invalid filter json: {error}")))?;
    filter
        .validate()
        .map_err(|error| AppError::Invalid(error.to_string()))?;
    Ok(filter)
}

/// Refuse un filtre et sa requête textuelle lorsqu'ils divergent.
pub fn coherent_filter(filter: FilterExpr, query: Option<&str>) -> Result<FilterExpr, AppError> {
    if let Some(raw) = query.map(str::trim).filter(|value| !value.is_empty())
        && parse_filter_query(raw)? != filter
    {
        return Err(AppError::Invalid("filter and query diverge".to_string()));
    }
    Ok(filter)
}

/// Filtre d'un formulaire de vue : JSON prioritaire, sinon requête textuelle.
pub fn resolve_view_filter(
    filter_json: Option<&str>,
    query: Option<&str>,
) -> Result<Option<FilterExpr>, AppError> {
    let raw_filter = filter_json.map(str::trim).filter(|value| !value.is_empty());
    let raw_query = query.map(str::trim).filter(|value| !value.is_empty());
    match (raw_filter, raw_query) {
        (Some(filter), query) => {
            let filter = parse_filter_json(filter)?;
            coherent_filter(filter, query).map(Some)
        }
        (None, Some(query)) => parse_filter_query(query).map(Some),
        (None, None) => Ok(None),
    }
}

impl App {
    pub fn list_views(&self) -> Result<Vec<SavedView>, AppError> {
        Ok(self.store.list_views()?)
    }

    pub fn get_view(&self, id: ViewId) -> Result<SavedView, AppError> {
        self.store
            .get_view(id)?
            .ok_or_else(|| AppError::NotFound(format!("view {}", id.0)))
    }

    pub fn views_with_counts(&self) -> Result<Vec<(SavedView, u64)>, AppError> {
        self.list_views()?
            .into_iter()
            .map(|view| self.count_filter(&view.filter).map(|count| (view, count)))
            .collect()
    }

    pub fn view_with_count(&self, id: ViewId) -> Result<(SavedView, u64), AppError> {
        let view = self.get_view(id)?;
        let count = self.count_filter(&view.filter)?;
        Ok((view, count))
    }

    pub fn create_view(&self, input: ViewInput) -> Result<SavedView, AppError> {
        self.create_view_in_group(input, None)
    }

    /// Crée la vue puis la place dans `group`, ou dans le groupe de repli.
    pub fn create_view_in_group(
        &self,
        input: ViewInput,
        group: Option<ViewGroupId>,
    ) -> Result<SavedView, AppError> {
        let (name, filter, icon) = validated_view_fields(&input)?;
        if let Some(group) = group {
            self.require_view_group(group)?;
        }
        let view = NewSavedView {
            name,
            filter,
            icon,
            color: input.color,
            // 0 = le store calcule `max(position) + 1`.
            position: input.position.unwrap_or(0),
            sort: input.sort,
        };
        let view = self.store.create_view(&view)?;
        let group = match group {
            Some(group) => group,
            None => self.fallback_view_group()?,
        };
        self.prepend_view(group, view.id)?;
        Ok(view)
    }

    pub fn update_view(&self, id: ViewId, input: ViewInput) -> Result<SavedView, AppError> {
        let (name, filter, icon) = validated_view_fields(&input)?;
        let patch = SavedViewPatch {
            name: Some(name),
            filter: Some(filter),
            icon: Some(icon),
            color: Some(input.color),
            position: input.position,
            sort: Some(input.sort),
        };
        Ok(self.store.update_view(id, &patch)?)
    }

    pub fn delete_view(&self, id: ViewId) -> Result<bool, AppError> {
        Ok(self.store.delete_view(id)?)
    }

    pub fn run_view(
        &self,
        id: ViewId,
        offset: usize,
        limit: usize,
    ) -> Result<Page<FileWithTags>, AppError> {
        validate_page_limit(limit)?;
        let page = self.list_files_page(&crate::FileListRequest {
            view: Some(id),
            offset,
            limit,
            ..crate::FileListRequest::new(limit)
        })?;
        Ok(page.into_page())
    }

    pub fn count_view(&self, id: ViewId) -> Result<u64, AppError> {
        let view = self.get_view(id)?;
        self.count_filter(&view.filter)
    }

    /// Appartenance ponctuelle d'un fichier à une vue, sans parcourir la liste.
    pub fn file_matches_view(
        &self,
        file: fs_ez_core::FileId,
        id: ViewId,
    ) -> Result<bool, AppError> {
        let view = self.get_view(id)?;
        Ok(self.store.file_matches_filter(file, &view.filter)?)
    }

    /// Valide le filtre et retourne sa forme canonique affichable.
    pub fn canonical_filter(&self, filter: &FilterExpr) -> Result<String, AppError> {
        filter.validate()?;
        Ok(pretty(filter))
    }

    pub fn count_filter(&self, filter: &FilterExpr) -> Result<u64, AppError> {
        Ok(self.store.count_matching(filter)?)
    }
}

fn required_filter(filter: Option<FilterExpr>) -> Result<FilterExpr, AppError> {
    let filter = filter.ok_or_else(|| AppError::Invalid("filter required".to_string()))?;
    filter.validate()?;
    Ok(filter)
}

#[cfg(test)]
mod tests;
