//! Assemblage des read models de liste : lignes et tri.

use fs_ez_core::{FileWithTags, SavedView, Sort, SortDir, SortField, VirtualFolderScope};

use crate::query_params::{FileFilter, ValidatedFiles, sort_token};

use super::labels::FilesLabels;
use super::urls::{ListQuery, files_url};
use super::{FilesData, FilesRowsData, SortHeader, rows};

fn list_path(view: Option<&SavedView>, folder: Option<VirtualFolderScope>) -> String {
    match folder {
        Some(VirtualFolderScope::Folder(folder)) => format!("/virtual-folders/{}", folder.0),
        Some(VirtualFolderScope::Root) => "/virtual-folders".to_string(),
        None => match view {
            Some(view) => format!("/views/{}", view.id),
            None => "/".to_string(),
        },
    }
}

pub fn build_files_data(
    labels: FilesLabels,
    items: Vec<FileWithTags>,
    total: u64,
    params: &ValidatedFiles,
    sort: Sort,
    view: Option<&SavedView>,
    folder: Option<VirtualFolderScope>,
) -> FilesData {
    let path = list_path(view, folder);
    let tags = params.tags.clone();
    let q = params.q.clone();
    let query = ListQuery::new(params, sort_token(sort));
    let refresh_query = query.with_view(params.view);
    /* Rafraîchissement complet puis fragments de scroll : même requête, base différente. */
    let refresh_url = files_url("/partials/files_page", &refresh_query);
    let rows_endpoint = files_url("/partials/files/rows", &refresh_query);
    let scope_url = files_url(&path, &query);
    let sort_headers = build_sort_headers(&labels, &path, &query, sort);

    FilesData {
        rows: rows(items),
        scope_url,
        total,
        q: q.unwrap_or_default(),
        tags,
        rows_url: rows_endpoint,
        refresh_url,
        refresh_on_tag_change: params.filter != Some(FileFilter::Untagged) || folder.is_some(),
        sort_headers,
        labels,
        draggable: folder.is_some(),
        virtual_folders: None,
    }
}

pub fn build_rows_data(
    labels: FilesLabels,
    items: Vec<FileWithTags>,
    draggable: bool,
) -> FilesRowsData {
    FilesRowsData {
        rows: rows(items),
        labels,
        draggable,
    }
}

fn build_sort_headers(
    labels: &FilesLabels,
    path: &str,
    query: &ListQuery<'_>,
    sort: Sort,
) -> Vec<SortHeader> {
    let mut sort_headers = Vec::new();
    let mut link_query = query.clone();
    for (key, label, field) in [
        ("name", &labels.name, Some(SortField::Name)),
        ("tags", &labels.tags, None),
        ("size", &labels.size, Some(SortField::Size)),
        ("date", &labels.date, Some(SortField::Mtime)),
    ] {
        let (url, active, sortable, aria_sort, arrow) = match field {
            None => (String::new(), false, false, "none", ""),
            Some(field) => {
                let active = sort.field == field;
                let next_dir = if active && sort.dir == SortDir::Asc {
                    SortDir::Desc
                } else {
                    SortDir::Asc
                };
                link_query.set_sort(sort_token(Sort {
                    field,
                    dir: next_dir,
                }));
                (
                    files_url(path, &link_query),
                    active,
                    true,
                    if !active {
                        "none"
                    } else if sort.dir == SortDir::Asc {
                        "ascending"
                    } else {
                        "descending"
                    },
                    if sort.dir == SortDir::Asc {
                        "↑"
                    } else {
                        "↓"
                    },
                )
            }
        };
        sort_headers.push(SortHeader {
            key,
            label: label.clone(),
            url,
            active,
            sortable,
            aria_sort,
            arrow,
        });
    }
    sort_headers
}
