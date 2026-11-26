//! Tests d'integration de `FileStore` et `TagStore` sur une base memoire.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use fs_ez_core::{FileId, FilePatch, Fingerprint, Sort, SortDir, SortField, TagColor, TagId};
use fs_ez_sqlite::SqliteStore;
use fs_ez_store::{FileListQuery, FileStore, GroupStore, StoreError, TagStore};
use time::macros::datetime;
use time::{Duration, OffsetDateTime};

mod common;

use common::{insert, list_query, new_file, store, tag};

fn query(sort: Sort, offset: usize, limit: usize) -> FileListQuery {
    list_query(None, sort, offset, limit)
}

fn sort(field: SortField, dir: SortDir) -> Sort {
    Sort { field, dir }
}

fn collect_ids(store: &SqliteStore, sort: Sort) -> Vec<i64> {
    let mut ids = Vec::new();
    let mut offset = 0;
    for _ in 0..10 {
        let page = store.list_files(&query(sort, offset, 2)).expect("page");
        ids.extend(page.items.iter().map(|item| item.file.id.0));
        match page.next_offset {
            Some(next) => offset = next,
            None => return ids,
        }
    }
    panic!("pagination sans fin");
}

#[path = "files_tags/bulk.rs"]
mod bulk;
#[path = "files_tags/counts.rs"]
mod counts;
#[path = "files_tags/files.rs"]
mod files;
#[path = "files_tags/queries.rs"]
mod queries;
#[path = "files_tags/schema.rs"]
mod schema;
#[path = "files_tags/tags.rs"]
mod tags;
