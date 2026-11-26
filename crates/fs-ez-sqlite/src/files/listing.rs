//! Listing et comptages de `FileStore`.

use fs_ez_core::{FileRecord, FileWithTags, FilterExpr, SortDir, SortField};
use fs_ez_store::{FileListQuery, Page, StoreError, StoreResult};
use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, params_from_iter};

use super::tags::tags_for_files;
use super::util::{count, cutoff, escape_like};
use crate::convert::{FILE_COLUMNS, file_from_row};
use crate::query_sql;
use crate::store::SqliteStore;
use crate::store::{collect_rows, db};

/// Prédicat partagé `untagged`, `f` = alias de `file`.
const UNTAGGED_SQL: &str = "NOT EXISTS (SELECT 1 FROM file_tag ft WHERE ft.file_id = f.id)";

/// Prédicat partagé `tagged_since`, `?` = date plancher.
const TAGGED_SINCE_SQL: &str =
    "EXISTS (SELECT 1 FROM file_tag ft WHERE ft.file_id = f.id AND ft.created_at >= ?)";

pub(super) fn list_files(
    store: &SqliteStore,
    query: &FileListQuery,
) -> StoreResult<Page<FileWithTags>> {
    query.validate()?;
    fs_ez_store::validate_page_limit(query.limit)?;
    store.with_conn(|conn| list_files_conn(conn, query))
}

pub(super) fn all_files(store: &SqliteStore) -> StoreResult<Vec<FileRecord>> {
    store.with_conn(|conn| {
        let sql = format!("SELECT {FILE_COLUMNS} FROM file f ORDER BY f.id");
        let mut statement = db(conn.prepare(&sql))?;
        collect_rows(&mut statement, [], file_from_row)
    })
}

pub(super) fn count_files(store: &SqliteStore) -> StoreResult<u64> {
    store.with_conn(|conn| count(conn, "SELECT COUNT(*) FROM file", []))
}

pub(super) fn count_untagged(store: &SqliteStore) -> StoreResult<u64> {
    let sql = format!("SELECT COUNT(*) FROM file f WHERE {UNTAGGED_SQL}");
    store.with_conn(|conn| count(conn, &sql, []))
}

pub(super) fn count_tagged_since(store: &SqliteStore, days: i64) -> StoreResult<u64> {
    let cutoff = cutoff(days);
    store.with_conn(|conn| {
        count(
            conn,
            "SELECT COUNT(DISTINCT file_id) FROM file_tag WHERE created_at >= ?1",
            [cutoff],
        )
    })
}

pub(super) fn count_matching(store: &SqliteStore, filter: &FilterExpr) -> StoreResult<u64> {
    let (subquery, values) = query_sql::filter_subquery(filter)?;
    let sql = format!("SELECT COUNT(*) FROM file f WHERE f.id IN ({subquery})");
    store.with_conn(|conn| count(conn, &sql, params_from_iter(values.iter())))
}

/// Même prédicats que `list_files`, sans offset ni limite.
pub(super) fn count_files_query(store: &SqliteStore, query: &FileListQuery) -> StoreResult<u64> {
    query.validate()?;
    let (where_sql, values) = conditions(query)?;
    let sql = format!("SELECT COUNT(*) FROM file f{where_sql}");
    store.with_conn(|conn| count(conn, &sql, params_from_iter(values.iter())))
}

pub(super) fn file_matches_filter(
    store: &SqliteStore,
    id: fs_ez_core::FileId,
    filter: &FilterExpr,
) -> StoreResult<bool> {
    let (predicate, values) = query_sql::filter_predicate(filter)?;
    let sql = format!("SELECT EXISTS(SELECT 1 FROM file f WHERE f.id = ? AND {predicate})");
    let values = std::iter::once(SqlValue::Integer(id.0)).chain(values);
    store.with_conn(|conn| {
        let found: i64 = db(conn.query_row(&sql, params_from_iter(values), |row| row.get(0)))?;
        Ok(found != 0)
    })
}

fn list_files_conn(conn: &Connection, query: &FileListQuery) -> StoreResult<Page<FileWithTags>> {
    let (where_sql, mut values) = conditions(query)?;
    let limit =
        i64::try_from(query.limit + 1).map_err(|_| StoreError::Invalid("limit".to_string()))?;
    let offset =
        i64::try_from(query.offset).map_err(|_| StoreError::Invalid("offset".to_string()))?;
    values.push(SqlValue::Integer(limit));
    values.push(SqlValue::Integer(offset));
    let sql = format!(
        "SELECT {FILE_COLUMNS} FROM file f{where_sql} ORDER BY {} LIMIT ? OFFSET ?",
        order_by(query)
    );
    let mut statement = db(conn.prepare(&sql))?;
    let mut files = collect_rows(
        &mut statement,
        params_from_iter(values.iter()),
        file_from_row,
    )?;
    let next_offset = if files.len() > query.limit {
        files.pop();
        Some(query.offset + query.limit)
    } else {
        None
    };
    let ids: Vec<_> = files.iter().map(|file| file.id).collect();
    let mut tags = tags_for_files(conn, &ids)?;
    let items = files
        .into_iter()
        .map(|file| FileWithTags {
            tags: tags.remove(&file.id).unwrap_or_default(),
            file,
        })
        .collect();
    Ok(Page { items, next_offset })
}

fn conditions(query: &FileListQuery) -> StoreResult<(String, Vec<SqlValue>)> {
    let mut predicates: Vec<String> = Vec::new();
    let mut values: Vec<SqlValue> = Vec::new();
    if query.untagged {
        predicates.push(UNTAGGED_SQL.to_string());
    }
    if let Some(days) = query.tagged_since_days {
        predicates.push(TAGGED_SINCE_SQL.to_string());
        values.push(SqlValue::Text(cutoff(days)));
    }
    if let Some(days) = query.modified_since_days {
        predicates.push("f.mtime >= ?".to_string());
        values.push(SqlValue::Text(cutoff(days)));
    }
    if let Some(scope) = query.virtual_folder {
        match scope.id() {
            Some(folder) => {
                predicates.push(
                    "EXISTS (SELECT 1 FROM virtual_folder_file vf
                             WHERE vf.file_id = f.id AND vf.folder_id = ?)"
                        .to_string(),
                );
                values.push(SqlValue::Integer(folder.0));
            }
            None => {
                predicates.push(
                    "NOT EXISTS (SELECT 1 FROM virtual_folder_file vf WHERE vf.file_id = f.id)"
                        .to_string(),
                );
            }
        }
    }
    if let Some(filter) = &query.filter {
        let (predicate, filter_values) = query_sql::filter_predicate(filter)?;
        predicates.push(predicate);
        values.extend(filter_values);
    }
    if let Some(text) = &query.text
        && !text.is_empty()
    {
        // `lower` SQLite ne traite que l'ASCII, conforme a la recherche insensible a la casse ASCII.
        predicates.push("lower(f.name) LIKE lower(?) ESCAPE '\\'".to_string());
        values.push(SqlValue::Text(format!("%{}%", escape_like(text))));
    }
    let where_sql = if predicates.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", predicates.join(" AND "))
    };
    Ok((where_sql, values))
}

/// Les modes récents imposent leur ordre : date de tag ou date de fichier, plus récent d'abord.
fn order_by(query: &FileListQuery) -> String {
    if query.tagged_since_days.is_some() {
        return "COALESCE((SELECT MAX(ft.created_at) FROM file_tag ft WHERE ft.file_id = f.id), '') DESC, f.id DESC".to_string();
    }
    if query.modified_since_days.is_some() {
        return "f.mtime DESC, f.id DESC".to_string();
    }
    let sort = &query.sort;
    let direction = if sort.dir == SortDir::Asc {
        "ASC"
    } else {
        "DESC"
    };
    match sort.field {
        SortField::Id => format!("f.id {direction}"),
        SortField::Name => format!("f.name COLLATE BINARY {direction}, f.id {direction}"),
        SortField::Size => format!("f.size {direction}, f.id {direction}"),
        SortField::Mtime => format!("f.mtime {direction}, f.id {direction}"),
    }
}
