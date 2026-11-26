//! Implementation de `TagStore` sur SQLite.

use fs_ez_core::{
    FilterExpr, GroupId, Tag, TagColor, TagCount, TagId, normalize_tag_name, validate_hex_color,
};
use fs_ez_store::{StoreError, StoreResult, TagStore};
use rusqlite::types::Type;
use rusqlite::{Connection, params, params_from_iter};
use time::OffsetDateTime;

use crate::convert::{
    TAG_COLUMNS, conversion, tag_color_to_str, tag_from_row, to_count, ts_to_str,
};
use crate::query_sql;
use crate::store::{SqliteStore, collect_rows, db, query_optional};

/// Jointure de groupe alignée sur l'ordre de parcours, partagée par `list_tags` et `tag_counts`.
const GROUP_JOIN: &str = "LEFT JOIN tag_group_member m ON m.tag_id = t.id
 LEFT JOIN tag_group g ON g.id = m.group_id";

/// Ordre commun : groupe, membre, nom normalisé.
const GROUP_ORDER: &str = "ORDER BY g.position, m.position, t.name_norm";

impl TagStore for SqliteStore {
    fn get_or_create_tag(&self, group: GroupId, name: &str, color: TagColor) -> StoreResult<Tag> {
        let (display, normalized) = normalize_tag_name(name)?;
        let now = ts_to_str(OffsetDateTime::now_utc());
        self.with_conn_mut(|conn| {
            let transaction = db(conn.transaction())?;
            let position: i64 = db(transaction.query_row(
                "SELECT COALESCE(MAX(position) + 1, 0) FROM tag_group_member WHERE group_id = ?1",
                [group.0],
                |row| row.get(0),
            ))?;
            let tag = match tag_by_norm(&transaction, &normalized)? {
                Some(tag) => tag,
                None => {
                    db(transaction.execute(
                        "INSERT INTO tag(name, name_norm, color, created_at, position)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![display, normalized, tag_color_to_str(color), now, position],
                    ))?;
                    tag_by_id(&transaction, TagId(transaction.last_insert_rowid()))?
                        .ok_or_else(|| StoreError::Db("inserted tag is missing".to_string()))?
                }
            };
            db(transaction.execute(
                "INSERT OR IGNORE INTO tag_group_member(group_id, tag_id, position)
                 VALUES (?1, ?2, ?3)",
                params![group.0, tag.id.0, position],
            ))?;
            db(transaction.commit())?;
            Ok(tag)
        })
    }

    fn get_tag(&self, id: TagId) -> StoreResult<Option<Tag>> {
        self.with_conn(|conn| tag_by_id(conn, id))
    }

    fn get_tag_by_name(&self, name: &str) -> StoreResult<Option<Tag>> {
        let (_, normalized) = normalize_tag_name(name)?;
        self.with_conn(|conn| tag_by_norm(conn, &normalized))
    }

    fn list_tags(&self) -> StoreResult<Vec<Tag>> {
        self.with_conn(|conn| {
            let sql = format!("SELECT {TAG_COLUMNS} FROM tag t {GROUP_JOIN} {GROUP_ORDER}");
            let mut statement = db(conn.prepare(&sql))?;
            collect_rows(&mut statement, [], |row| tag_from_row(row, 0))
        })
    }

    fn rename_tag(&self, id: TagId, name: &str) -> StoreResult<Tag> {
        let (display, normalized) = normalize_tag_name(name)?;
        self.with_conn(|conn| {
            let changed = db(conn.execute(
                "UPDATE tag SET name = ?1, name_norm = ?2 WHERE id = ?3",
                params![display, normalized, id.0],
            ))?;
            if changed == 0 {
                return Err(StoreError::NotFound("tag"));
            }
            tag_by_id(conn, id)?.ok_or(StoreError::NotFound("tag"))
        })
    }

    fn set_tag_color(
        &self,
        id: TagId,
        color: TagColor,
        custom: Option<String>,
    ) -> StoreResult<Tag> {
        let custom = match custom {
            Some(raw) => Some(validate_hex_color(&raw)?),
            None => None,
        };
        self.with_conn(|conn| {
            let changed = db(conn.execute(
                "UPDATE tag SET color = ?1, color_custom = ?2 WHERE id = ?3",
                params![tag_color_to_str(color), custom, id.0],
            ))?;
            if changed == 0 {
                return Err(StoreError::NotFound("tag"));
            }
            tag_by_id(conn, id)?.ok_or(StoreError::NotFound("tag"))
        })
    }

    fn delete_tag(&self, id: TagId) -> StoreResult<bool> {
        self.with_conn_mut(|conn| {
            let transaction = db(conn.transaction())?;
            let Some(tag) = tag_by_id(&transaction, id)? else {
                return Ok(false);
            };
            db(transaction.execute("DELETE FROM tag WHERE id = ?1", [id.0]))?;
            remove_tag_from_views(&transaction, &tag.name_norm)?;
            db(transaction.commit())?;
            Ok(true)
        })
    }

    fn merge_tags(&self, source: TagId, target: TagId) -> StoreResult<u64> {
        if source == target {
            return Err(StoreError::Invalid(
                "cannot merge a tag into itself".to_string(),
            ));
        }
        self.with_conn_mut(|conn| {
            let transaction = db(conn.transaction())?;
            let touched: i64 = db(transaction.query_row(
                "SELECT COUNT(DISTINCT file_id) FROM file_tag WHERE tag_id = ?1",
                [source.0],
                |row| row.get(0),
            ))?;
            db(transaction.execute(
                "INSERT INTO file_tag(file_id, tag_id, explicit, created_at)
                 SELECT file_id, ?1, explicit, created_at FROM file_tag WHERE tag_id = ?2
                 ON CONFLICT DO NOTHING",
                params![target.0, source.0],
            ))?;
            let deleted = db(transaction.execute("DELETE FROM tag WHERE id = ?1", [source.0]))?;
            if deleted == 0 {
                return Err(StoreError::NotFound("tag"));
            }
            db(transaction.commit())?;
            to_count(touched)
        })
    }

    fn tags_for_filter(&self, filter: &FilterExpr) -> StoreResult<Vec<Tag>> {
        let (subquery, values) = query_sql::filter_subquery(filter)?;
        let sql = format!(
            "SELECT DISTINCT {TAG_COLUMNS} FROM tag t
             JOIN file_tag ft ON ft.tag_id = t.id
             WHERE ft.file_id IN ({subquery})
             ORDER BY t.name_norm"
        );
        self.with_conn(|conn| {
            let mut statement = db(conn.prepare(&sql))?;
            collect_rows(&mut statement, params_from_iter(values.iter()), |row| {
                tag_from_row(row, 0)
            })
        })
    }

    fn tag_counts(&self) -> StoreResult<Vec<TagCount>> {
        self.with_conn(|conn| {
            let sql = format!(
                "SELECT {TAG_COLUMNS}, COUNT(DISTINCT ft.file_id)
                 FROM tag t
                 LEFT JOIN file_tag ft ON ft.tag_id = t.id
                 {GROUP_JOIN}
                 GROUP BY t.id {GROUP_ORDER}"
            );
            let mut statement = db(conn.prepare(&sql))?;
            collect_rows(&mut statement, [], |row| {
                let tag = tag_from_row(row, 0)?;
                let count: i64 = row.get(6)?;
                let count = to_count(count).map_err(|error| conversion(6, Type::Integer, error))?;
                Ok(TagCount { tag, count })
            })
        })
    }
}

fn remove_tag_from_views(conn: &Connection, tag_norm: &str) -> StoreResult<()> {
    let mut statement = db(conn.prepare("SELECT id, filter_json FROM saved_view"))?;
    let rows: Vec<(i64, String)> =
        collect_rows(&mut statement, [], |row| Ok((row.get(0)?, row.get(1)?)))?;
    drop(statement);
    let now = ts_to_str(OffsetDateTime::now_utc());
    for (id, raw) in rows {
        // Un filtre corrompu est ignoré pour ne pas bloquer la suppression du tag.
        let Ok(filter) = serde_json::from_str::<FilterExpr>(&raw) else {
            continue;
        };
        let Some(remaining) = filter.without_tag(tag_norm) else {
            db(conn.execute("DELETE FROM saved_view WHERE id = ?1", [id]))?;
            continue;
        };
        if remaining == filter {
            continue;
        }
        let filter_json = serde_json::to_string(&remaining)
            .map_err(|error| StoreError::Db(format!("invalid filter: {error}")))?;
        db(conn.execute(
            "UPDATE saved_view SET filter_json = ?1, updated_at = ?2 WHERE id = ?3",
            params![filter_json, now, id],
        ))?;
    }
    Ok(())
}

fn tag_by_id(conn: &Connection, id: TagId) -> StoreResult<Option<Tag>> {
    let sql = format!("SELECT {TAG_COLUMNS} FROM tag t WHERE t.id = ?1");
    query_optional(conn, &sql, [id.0], |row| tag_from_row(row, 0))
}

fn tag_by_norm(conn: &Connection, normalized: &str) -> StoreResult<Option<Tag>> {
    let sql = format!("SELECT {TAG_COLUMNS} FROM tag t WHERE t.name_norm = ?1");
    query_optional(conn, &sql, [normalized], |row| tag_from_row(row, 0))
}
