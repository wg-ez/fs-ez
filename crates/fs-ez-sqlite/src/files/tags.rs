//! Lecture des tags d'un fichier ou d'une page entière.

use std::collections::HashMap;

use fs_ez_core::{AppliedTag, FileId};
use fs_ez_store::StoreResult;
use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, params_from_iter};

use crate::convert::{applied_tag_from_row, applied_tag_from_row_at};
use crate::files::util::placeholders;
use crate::store::SqliteStore;
use crate::store::{collect_rows, db};

/// Colonnes attendues par `applied_tag_from_row_at`, à partir de `t.id`.
const APPLIED_TAG_COLUMNS: &str =
    "t.id, t.name, t.name_norm, t.color, t.color_custom, t.created_at, ft.explicit, ft.created_at";

const TAG_JOIN: &str = "FROM file_tag ft JOIN tag t ON t.id = ft.tag_id";

pub(super) fn tags_for_file(store: &SqliteStore, id: FileId) -> StoreResult<Vec<AppliedTag>> {
    store.with_conn(|conn| tags_for_conn(conn, id))
}

pub(super) fn tags_for_conn(conn: &Connection, id: FileId) -> StoreResult<Vec<AppliedTag>> {
    let sql = format!(
        "SELECT {APPLIED_TAG_COLUMNS} {TAG_JOIN} WHERE ft.file_id = ?1 ORDER BY t.name_norm"
    );
    let mut statement = db(conn.prepare(&sql))?;
    collect_rows(&mut statement, [id.0], applied_tag_from_row)
}

/// Tous les tags d'une page en une requête, ordre stable par fichier puis par nom.
pub(super) fn tags_for_files(
    conn: &Connection,
    ids: &[FileId],
) -> StoreResult<HashMap<FileId, Vec<AppliedTag>>> {
    let mut grouped: HashMap<FileId, Vec<AppliedTag>> = HashMap::with_capacity(ids.len());
    if ids.is_empty() {
        return Ok(grouped);
    }
    let sql = format!(
        "SELECT ft.file_id, {APPLIED_TAG_COLUMNS} {TAG_JOIN}
         WHERE ft.file_id IN ({}) ORDER BY ft.file_id, t.name_norm",
        placeholders(ids.len())
    );
    let values: Vec<SqlValue> = ids.iter().map(|id| SqlValue::Integer(id.0)).collect();
    let mut statement = db(conn.prepare(&sql))?;
    let rows = db(statement.query_map(params_from_iter(values.iter()), |row| {
        let file_id: i64 = row.get(0)?;
        let tag = applied_tag_from_row_at(row, 1)?;
        Ok((file_id, tag))
    }))?;
    for row in rows {
        let (file_id, tag) = db(row)?;
        grouped.entry(FileId(file_id)).or_default().push(tag);
    }
    Ok(grouped)
}
