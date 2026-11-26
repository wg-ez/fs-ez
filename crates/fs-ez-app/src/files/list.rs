//! Cas d'usage unique de lecture d'une liste de fichiers, partagé par l'UI, l'API et la CLI.

use fs_ez_core::{FileWithTags, FilterExpr, SavedView, Sort, ViewId, VirtualFolderScope};
use fs_ez_store::{FileListQuery, Page};

use crate::app::App;
use crate::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileListMode {
    #[default]
    All,
    Untagged,
    Recent,
    RecentFiles,
}

/// Requête normalisée : les adaptateurs décident de la provenance, ce type décide du sens.
#[derive(Debug, Clone)]
pub struct FileListRequest {
    pub view: Option<ViewId>,
    pub filter: Option<FilterExpr>,
    pub tags: Vec<String>,
    pub text: Option<String>,
    /// Fichiers d'un dossier virtuel, `Root` pour la racine.
    pub virtual_folder: Option<VirtualFolderScope>,
    pub mode: FileListMode,
    /// Fenêtre de `Recent`, sinon réglage global.
    pub recent_days: Option<i64>,
    /// Fenêtre de `RecentFiles`, sinon réglage global.
    pub recent_file_days: Option<i64>,
    pub sort: Option<Sort>,
    /// Saut direct du scroll virtuel.
    pub offset: usize,
    pub limit: usize,
    pub total: bool,
}

impl FileListRequest {
    pub fn new(limit: usize) -> Self {
        Self {
            view: None,
            filter: None,
            tags: Vec::new(),
            text: None,
            virtual_folder: None,
            mode: FileListMode::All,
            recent_days: None,
            recent_file_days: None,
            sort: None,
            offset: 0,
            limit,
            total: false,
        }
    }
}

/// Résultat commun : l'UI ajoute le rendu, l'API les DTO, la CLI le texte.
#[derive(Debug, Clone)]
pub struct FileListPage {
    pub items: Vec<FileWithTags>,
    pub next_offset: Option<usize>,
    pub total: Option<u64>,
    pub sort: Sort,
    pub view: Option<SavedView>,
}

impl FileListPage {
    /// Abandonne les métadonnées de rendu, ne garde que la page stockée.
    pub fn into_page(self) -> Page<FileWithTags> {
        Page {
            items: self.items,
            next_offset: self.next_offset,
        }
    }
}

impl App {
    /// Résout vue, défauts, filtres et mode une seule fois pour toutes les façades.
    pub fn list_files_page(&self, request: &FileListRequest) -> Result<FileListPage, AppError> {
        if request.offset > 1_000_000 {
            return Err(AppError::Invalid("offset too large".to_string()));
        }
        let settings = self.settings()?;
        let view = match request.view {
            Some(id) => Some(self.get_view(id)?),
            None => None,
        };
        let sort = request.sort.unwrap_or(match &view {
            Some(view) => view.sort,
            None => settings.default_sort,
        });
        let mut filters: Vec<FilterExpr> = Vec::new();
        if let Some(view) = &view {
            filters.push(view.filter.clone());
        }
        if let Some(filter) = &request.filter {
            filters.push(filter.clone());
        }
        if let Some(filter) = FilterExpr::all_tags(&request.tags) {
            filters.push(filter);
        }
        if request.mode == FileListMode::Untagged && !filters.is_empty() {
            return Err(AppError::Invalid(
                "untagged cannot be combined with a filter".to_string(),
            ));
        }
        let mut query = match request.mode {
            FileListMode::All => FileListQuery::new(sort, request.limit),
            FileListMode::Untagged => FileListQuery::untagged(sort, request.limit),
            FileListMode::Recent => FileListQuery::recent(
                request.recent_days.unwrap_or(settings.recent_days),
                sort,
                request.limit,
            ),
            FileListMode::RecentFiles => FileListQuery::recent_files(
                request
                    .recent_file_days
                    .unwrap_or(settings.recent_file_days),
                sort,
                request.limit,
            ),
        };
        query.filter = FilterExpr::and_all(filters);
        query.virtual_folder = request.virtual_folder;
        query.text = request
            .text
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string);
        query.offset = request.offset;
        let page = self.store.list_files(&query)?;
        let total = if request.total {
            Some(self.store.count_files_query(&query)?)
        } else {
            None
        };
        Ok(FileListPage {
            items: page.items,
            next_offset: page.next_offset,
            total,
            sort,
            view,
        })
    }
}
