//! Mutations des tags d'un fichier : ajout, retrait, remplacement.

use fs_ez_core::{FileId, TagId};
use fs_ez_store::StoreResult;
use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, params, params_from_iter};
use time::OffsetDateTime;

use crate::convert::ts_to_str;
use crate::files::util::{chunk_len, placeholders};
use crate::store::SqliteStore;
use crate::store::db;
use crate::store::with_immediate;

const INSERT_FILE_TAG_SQL: &str =
    "INSERT INTO file_tag(file_id, tag_id, explicit, created_at) VALUES (?1, ?2, ?3, ?4)";

/// Suffixe commun aux deux insertions, un explicite existant n'est jamais redescendu.
const UPSERT_EXPLICIT_SQL: &str = "ON CONFLICT(file_id, tag_id)
     DO UPDATE SET explicit = MAX(file_tag.explicit, excluded.explicit)
     WHERE file_tag.explicit < excluded.explicit";

const DELETE_FILE_TAG_BY_TAG_PREFIX: &str =
    "DELETE FROM file_tag WHERE tag_id = ?1 AND file_id IN (";

const DELETE_FILE_TAG_BY_FILE_PREFIX: &str = "DELETE FROM file_tag WHERE file_id IN (";

fn explicit_flag(explicit: bool) -> i64 {
    if explicit { 1 } else { 0 }
}

pub(in crate::files) fn apply_tag(
    store: &SqliteStore,
    file: FileId,
    tag: TagId,
    explicit: bool,
    at: OffsetDateTime,
) -> StoreResult<bool> {
    let sql = format!("{INSERT_FILE_TAG_SQL} {UPSERT_EXPLICIT_SQL}");
    store.with_conn(|conn| {
        let changed = db(conn.execute(
            &sql,
            params![file.0, tag.0, explicit_flag(explicit), ts_to_str(at)],
        ))?;
        Ok(changed > 0)
    })
}

pub(in crate::files) fn remove_tag(
    store: &SqliteStore,
    file: FileId,
    tag: TagId,
) -> StoreResult<bool> {
    store.with_conn(|conn| {
        let changed = db(conn.execute(
            "DELETE FROM file_tag WHERE file_id = ?1 AND tag_id = ?2",
            params![file.0, tag.0],
        ))?;
        Ok(changed > 0)
    })
}

/// `INSERT ... SELECT` par tag et tranche, promotion `explicit` sans écraser un explicite existant.
pub(in crate::files) fn add_tags_batch(
    store: &SqliteStore,
    files: &[FileId],
    tags: &[TagId],
    explicit: bool,
    at: OffsetDateTime,
) -> StoreResult<usize> {
    if files.is_empty() || tags.is_empty() {
        return Ok(0);
    }
    store.with_conn_mut(|conn| {
        with_immediate(conn, |transaction| {
            add_tags_tx(transaction, files, tags, explicit, at)
        })
    })
}

fn add_tags_tx(
    conn: &Connection,
    files: &[FileId],
    tags: &[TagId],
    explicit: bool,
    at: OffsetDateTime,
) -> StoreResult<usize> {
    let explicit = explicit_flag(explicit);
    let at = ts_to_str(at);
    let mut changed = 0;
    for tag in tags {
        for chunk in files.chunks(chunk_len(3)) {
            let sql = format!(
                "INSERT INTO file_tag(file_id, tag_id, explicit, created_at)
                 SELECT f.id, ?1, ?2, ?3 FROM file f WHERE f.id IN ({})
                 {UPSERT_EXPLICIT_SQL}",
                placeholders(chunk.len())
            );
            let values = std::iter::once(SqlValue::Integer(tag.0))
                .chain(std::iter::once(SqlValue::Integer(explicit)))
                .chain(std::iter::once(SqlValue::Text(at.clone())))
                .chain(chunk.iter().map(|id| SqlValue::Integer(id.0)));
            changed += db(conn.execute(&sql, params_from_iter(values)))?;
        }
    }
    Ok(changed)
}

/// `DELETE ... tag_id + file_id IN` par tag et tranche.
pub(in crate::files) fn remove_tags_batch(
    store: &SqliteStore,
    files: &[FileId],
    tags: &[TagId],
) -> StoreResult<usize> {
    if files.is_empty() || tags.is_empty() {
        return Ok(0);
    }
    store.with_conn_mut(|conn| {
        with_immediate(conn, |transaction| {
            let mut removed = 0;
            for tag in tags {
                for chunk in files.chunks(chunk_len(1)) {
                    let sql = format!(
                        "{DELETE_FILE_TAG_BY_TAG_PREFIX}{})",
                        placeholders(chunk.len())
                    );
                    let values = std::iter::once(SqlValue::Integer(tag.0))
                        .chain(chunk.iter().map(|id| SqlValue::Integer(id.0)));
                    removed += db(transaction.execute(&sql, params_from_iter(values)))?;
                }
            }
            Ok(removed)
        })
    })
}

/// Remplacement transactionnel : purge `file_tag` des fichiers puis ajout explicite.
pub(in crate::files) fn replace_tags_batch(
    store: &SqliteStore,
    files: &[FileId],
    tags: &[TagId],
    at: OffsetDateTime,
) -> StoreResult<usize> {
    if files.is_empty() {
        return Ok(0);
    }
    store.with_conn_mut(|conn| {
        with_immediate(conn, |transaction| {
            for chunk in files.chunks(chunk_len(0)) {
                let sql = format!(
                    "{DELETE_FILE_TAG_BY_FILE_PREFIX}{})",
                    placeholders(chunk.len())
                );
                let values = chunk.iter().map(|id| SqlValue::Integer(id.0));
                db(transaction.execute(&sql, params_from_iter(values)))?;
            }
            add_tags_tx(transaction, files, tags, true, at)
        })
    })
}
