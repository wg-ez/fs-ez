//! Ecritures et lectures unitaires de `FileStore`.

mod tags;

pub(super) use tags::{
    add_tags_batch, apply_tag, remove_tag, remove_tags_batch, replace_tags_batch,
};

use std::path::{Path, PathBuf};

use fs_ez_core::{FileId, FilePatch, FileRecord, Fingerprint, NewFile};
use fs_ez_store::{StoreError, StoreResult};
use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, params_from_iter};
use time::OffsetDateTime;

use crate::convert::{FILE_COLUMNS, file_from_row, file_kind_to_str, path_to_blob, ts_to_str};
use crate::files::util::{PARAM_BATCH, chunk_len, placeholders};
use crate::store::SqliteStore;
use crate::store::collect_rows;
use crate::store::db;
use crate::store::delete_by_id;
use crate::store::query_optional;
use crate::store::set;
use crate::store::with_immediate;

const INSERT_FILE_SQL: &str =
    "INSERT INTO file (path, path_display, name, dir, size, mtime, kind, fingerprint, created_at, updated_at)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)";

pub(super) fn insert_file(store: &SqliteStore, new: &NewFile) -> StoreResult<FileRecord> {
    let now = ts_to_str(OffsetDateTime::now_utc());
    let values = new_file_values(new, &now)?;
    store.with_conn(|conn| {
        db(conn.execute(INSERT_FILE_SQL, params_from_iter(values.iter())))?;
        file_by_id(conn, FileId(conn.last_insert_rowid()))?
            .ok_or_else(|| StoreError::Db("inserted file is missing".to_string()))
    })
}

/// Valeurs de `INSERT_FILE_SQL`, ordre des colonnes unique, `created_at` et `updated_at` partagent `?9`.
fn new_file_values(new: &NewFile, now: &str) -> StoreResult<Vec<SqlValue>> {
    let size = i64::try_from(new.size)
        .map_err(|_| StoreError::Invalid("file size exceeds i64".to_string()))?;
    Ok(vec![
        SqlValue::Blob(path_to_blob(&new.path)),
        SqlValue::Text(new.path_display.clone()),
        SqlValue::Text(new.name.clone()),
        SqlValue::Text(new.dir.clone()),
        SqlValue::Integer(size),
        SqlValue::Text(ts_to_str(new.mtime)),
        SqlValue::Text(file_kind_to_str(new.kind).to_string()),
        new.fingerprint
            .as_ref()
            .map_or(SqlValue::Null, |fp| SqlValue::Text(fp.as_str().to_string())),
        SqlValue::Text(now.to_string()),
    ])
}

pub(super) fn update_file(
    store: &SqliteStore,
    id: FileId,
    patch: &FilePatch,
) -> StoreResult<FileRecord> {
    store.with_conn(|conn| update_file_conn(conn, id, patch))
}

/// Un seul `UPDATE ... WHERE id = ? AND path = ?` : la ligne n'est réécrite que si le chemin n'a pas bougé.
pub(super) fn update_file_path_if(
    store: &SqliteStore,
    id: FileId,
    expected_path: &Path,
    patch: &FilePatch,
) -> StoreResult<bool> {
    let (sets, mut values) = patch_sets(patch)?;
    values.push(SqlValue::Integer(id.0));
    values.push(SqlValue::Blob(path_to_blob(expected_path)));
    let sql = patch_update_sql(&sets, "id = ? AND path = ?");
    store.with_conn(|conn| {
        let changed = db(conn.execute(&sql, params_from_iter(values.iter())))?;
        Ok(changed == 1)
    })
}

/// Une transaction `BEGIN IMMEDIATE`, une mise à jour par entrée, rollback global.
pub(super) fn update_files_batch(
    store: &SqliteStore,
    updates: &[(FileId, FilePatch)],
) -> StoreResult<usize> {
    if updates.is_empty() {
        return Ok(0);
    }
    store.with_conn_mut(|conn| {
        with_immediate(conn, |transaction| {
            for (id, patch) in updates {
                apply_patch(transaction, *id, patch)?;
            }
            Ok(updates.len())
        })
    })
}

fn update_file_conn(conn: &Connection, id: FileId, patch: &FilePatch) -> StoreResult<FileRecord> {
    apply_patch(conn, id, patch)?;
    file_by_id(conn, id)?.ok_or(StoreError::NotFound("file"))
}

fn apply_patch(conn: &Connection, id: FileId, patch: &FilePatch) -> StoreResult<()> {
    let (sets, mut values) = patch_sets(patch)?;
    values.push(SqlValue::Integer(id.0));
    let sql = patch_update_sql(&sets, "id = ?");
    let changed = db(conn.execute(&sql, params_from_iter(values.iter())))?;
    if changed == 0 {
        return Err(StoreError::NotFound("file"));
    }
    Ok(())
}

/// `UPDATE file` à partir des affectations de `patch_sets`, ordre des valeurs conservé.
fn patch_update_sql(sets: &[String], where_sql: &str) -> String {
    format!("UPDATE file SET {} WHERE {where_sql}", sets.join(", "))
}

/// Colonnes du patch et `updated_at`, ordre des valeurs aligné sur les affectations `SET`.
fn patch_sets(patch: &FilePatch) -> StoreResult<(Vec<String>, Vec<SqlValue>)> {
    let mut sets: Vec<String> = Vec::new();
    let mut values: Vec<SqlValue> = Vec::new();
    if let Some(path) = &patch.path {
        set(
            &mut sets,
            &mut values,
            "path",
            SqlValue::Blob(path_to_blob(path)),
        );
    }
    if let Some(path_display) = &patch.path_display {
        set(
            &mut sets,
            &mut values,
            "path_display",
            SqlValue::Text(path_display.clone()),
        );
    }
    if let Some(name) = &patch.name {
        set(&mut sets, &mut values, "name", SqlValue::Text(name.clone()));
    }
    if let Some(dir) = &patch.dir {
        set(&mut sets, &mut values, "dir", SqlValue::Text(dir.clone()));
    }
    if let Some(size) = patch.size {
        let size = i64::try_from(size)
            .map_err(|_| StoreError::Invalid("file size exceeds i64".to_string()))?;
        set(&mut sets, &mut values, "size", SqlValue::Integer(size));
    }
    if let Some(mtime) = patch.mtime {
        set(
            &mut sets,
            &mut values,
            "mtime",
            SqlValue::Text(ts_to_str(mtime)),
        );
    }
    if let Some(kind) = patch.kind {
        set(
            &mut sets,
            &mut values,
            "kind",
            SqlValue::Text(file_kind_to_str(kind).to_string()),
        );
    }
    if let Some(fingerprint) = &patch.fingerprint {
        let value = match fingerprint {
            Some(value) => SqlValue::Text(value.as_str().to_string()),
            None => SqlValue::Null,
        };
        set(&mut sets, &mut values, "fingerprint", value);
    }
    set(
        &mut sets,
        &mut values,
        "updated_at",
        SqlValue::Text(ts_to_str(OffsetDateTime::now_utc())),
    );
    Ok((sets, values))
}

pub(super) fn get_file(store: &SqliteStore, id: FileId) -> StoreResult<Option<FileRecord>> {
    store.with_conn(|conn| file_by_id(conn, id))
}

/// Une requête `IN` par tranche, ordre d'identifiant stable pour les tests.
pub(super) fn get_files_by_ids(
    store: &SqliteStore,
    ids: &[FileId],
) -> StoreResult<Vec<FileRecord>> {
    store.with_conn(|conn| {
        select_files_in(conn, ids, "id", " ORDER BY f.id", |id| {
            SqlValue::Integer(id.0)
        })
    })
}

/// `SELECT ... WHERE f.{column} IN (...)` par tranche, `suffix` = prédicat ou ordre propre à l'appelant.
fn select_files_in<T>(
    conn: &Connection,
    items: &[T],
    column: &str,
    suffix: &str,
    value: impl Fn(&T) -> SqlValue,
) -> StoreResult<Vec<FileRecord>> {
    let mut found = Vec::with_capacity(items.len());
    for chunk in items.chunks(PARAM_BATCH) {
        let sql = format!(
            "SELECT {FILE_COLUMNS} FROM file f WHERE f.{column} IN ({}){suffix}",
            placeholders(chunk.len())
        );
        let mut statement = db(conn.prepare(&sql))?;
        found.extend(collect_rows(
            &mut statement,
            params_from_iter(chunk.iter().map(&value)),
            file_from_row,
        )?);
    }
    Ok(found)
}

pub(super) fn find_file_by_path(
    store: &SqliteStore,
    path: &Path,
) -> StoreResult<Option<FileRecord>> {
    let blob = path_to_blob(path);
    store.with_conn(|conn| {
        let sql = format!("SELECT {FILE_COLUMNS} FROM file f WHERE f.path = ?1");
        query_optional(conn, &sql, [blob], file_from_row)
    })
}

pub(super) fn find_file_by_name(
    store: &SqliteStore,
    name: &str,
) -> StoreResult<Option<FileRecord>> {
    store.with_conn(|conn| {
        let sql =
            format!("SELECT {FILE_COLUMNS} FROM file f WHERE f.name = ?1 ORDER BY f.id LIMIT 1");
        query_optional(conn, &sql, [name], file_from_row)
    })
}

pub(super) fn find_file_by_fingerprint(
    store: &SqliteStore,
    fp: &Fingerprint,
) -> StoreResult<Option<FileRecord>> {
    store.with_conn(|conn| {
        // Prédicat répété, sans lui SQLite ne prouve pas l'index partiel → scan complet de la table.
        let sql = format!(
            "SELECT {FILE_COLUMNS} FROM file f
             WHERE f.fingerprint = ?1 AND f.fingerprint IS NOT NULL AND f.fingerprint <> ''"
        );
        query_optional(conn, &sql, [fp.as_str()], file_from_row)
    })
}

pub(super) fn find_files_by_fingerprints(
    store: &SqliteStore,
    fps: &[Fingerprint],
) -> StoreResult<Vec<FileRecord>> {
    // Même prédicat que l'unité : indispensable pour `file_fingerprint_unique`.
    store.with_conn(|conn| {
        select_files_in(
            conn,
            fps,
            "fingerprint",
            " AND f.fingerprint IS NOT NULL AND f.fingerprint <> ''",
            |fp| SqlValue::Text(fp.as_str().to_string()),
        )
    })
}

pub(super) fn find_files_by_paths(
    store: &SqliteStore,
    paths: &[PathBuf],
) -> StoreResult<Vec<FileRecord>> {
    store.with_conn(|conn| {
        select_files_in(conn, paths, "path", "", |path| {
            SqlValue::Blob(path_to_blob(path))
        })
    })
}

/// Une transaction `BEGIN IMMEDIATE`, une requête préparée, aucun `file_by_id` par ligne.
pub(super) fn insert_files_batch(
    store: &SqliteStore,
    files: &[NewFile],
) -> StoreResult<Vec<FileId>> {
    if files.is_empty() {
        return Ok(Vec::new());
    }
    store.with_conn_mut(|conn| {
        with_immediate(conn, |transaction| {
            let now = ts_to_str(OffsetDateTime::now_utc());
            let mut ids = Vec::with_capacity(files.len());
            {
                let mut statement = db(transaction.prepare(INSERT_FILE_SQL))?;
                for new in files {
                    let values = new_file_values(new, &now)?;
                    db(statement.execute(params_from_iter(values.iter())))?;
                    ids.push(FileId(transaction.last_insert_rowid()));
                }
            }
            Ok(ids)
        })
    })
}

pub(super) fn delete_file(store: &SqliteStore, id: FileId) -> StoreResult<bool> {
    store.with_conn(|conn| delete_by_id(conn, "file", id.0))
}

/// Une transaction `BEGIN IMMEDIATE`, un `DELETE ... IN` par tranche, cascade `file_tag`.
pub(super) fn delete_files_batch(store: &SqliteStore, ids: &[FileId]) -> StoreResult<usize> {
    if ids.is_empty() {
        return Ok(0);
    }
    store.with_conn_mut(|conn| {
        with_immediate(conn, |transaction| {
            let mut deleted = 0;
            for chunk in ids.chunks(chunk_len(0)) {
                let sql = format!(
                    "DELETE FROM file WHERE id IN ({})",
                    placeholders(chunk.len())
                );
                let values = chunk.iter().map(|id| SqlValue::Integer(id.0));
                deleted += db(transaction.execute(&sql, params_from_iter(values)))?;
            }
            Ok(deleted)
        })
    })
}

fn file_by_id(conn: &Connection, id: FileId) -> StoreResult<Option<FileRecord>> {
    let sql = format!("SELECT {FILE_COLUMNS} FROM file f WHERE f.id = ?1");
    query_optional(conn, &sql, [id.0], file_from_row)
}
