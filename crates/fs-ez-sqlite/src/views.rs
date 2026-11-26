//! Implementation de `ViewStore` sur SQLite.

use fs_ez_core::{
    FilterExpr, NewSavedView, SavedView, SavedViewPatch, Sort, ViewId, normalize_view_name,
};
use fs_ez_store::{StoreError, StoreResult, ViewStore};
use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, Row, params, params_from_iter};
use time::OffsetDateTime;

use crate::convert::{tag_color_from_str, tag_color_to_str, ts_from_str, ts_to_str};
use crate::store::{SqliteStore, collect_rows, db, delete_by_id, query_optional, set};

pub(crate) const VIEW_COLUMNS: &str =
    "id, name, filter_json, icon, color, position, sort, created_at, updated_at";

fn view_name(raw: &str) -> StoreResult<String> {
    normalize_view_name(raw)
        .ok_or_else(|| StoreError::Invalid(format!("invalid view name {raw:?}")))
}

fn encode_filter(filter: &FilterExpr) -> StoreResult<String> {
    serde_json::to_string(filter)
        .map_err(|error| StoreError::Invalid(format!("invalid filter: {error}")))
}

/// Colonnes brutes, decodees hors du mapper pour garder les messages d'erreur du domaine.
pub(crate) struct RawView {
    id: i64,
    name: String,
    filter_json: String,
    icon: Option<String>,
    color: Option<String>,
    position: i64,
    sort: String,
    created_at: String,
    updated_at: String,
}

pub(crate) fn raw_view_from_row(row: &Row<'_>) -> rusqlite::Result<RawView> {
    Ok(RawView {
        id: row.get(0)?,
        name: row.get(1)?,
        filter_json: row.get(2)?,
        icon: row.get(3)?,
        color: row.get(4)?,
        position: row.get(5)?,
        sort: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

pub(crate) fn view_from_raw(raw: RawView) -> StoreResult<SavedView> {
    let filter = serde_json::from_str(&raw.filter_json)
        .map_err(|_| StoreError::Db(format!("corrupt filter_json for view {}", raw.id)))?;
    Ok(SavedView {
        id: ViewId(raw.id),
        name: raw.name,
        filter,
        icon: raw.icon,
        color: match raw.color {
            Some(token) => Some(tag_color_from_str(&token)?),
            None => None,
        },
        position: raw.position,
        sort: Sort::decode(&raw.sort)
            .ok_or_else(|| StoreError::Invalid("invalid sort".to_string()))?,
        created_at: ts_from_str(&raw.created_at)?,
        updated_at: ts_from_str(&raw.updated_at)?,
    })
}

fn view_by_id(conn: &Connection, id: ViewId) -> StoreResult<Option<SavedView>> {
    let sql = format!("SELECT {VIEW_COLUMNS} FROM saved_view WHERE id = ?1");
    let raw: Option<RawView> = query_optional(conn, &sql, [id.0], raw_view_from_row)?;
    raw.map(view_from_raw).transpose()
}

fn view_by_name(conn: &Connection, name: &str) -> StoreResult<Option<SavedView>> {
    let sql = format!("SELECT {VIEW_COLUMNS} FROM saved_view WHERE name = ?1");
    let raw: Option<RawView> = query_optional(conn, &sql, [name], raw_view_from_row)?;
    raw.map(view_from_raw).transpose()
}

fn next_position(conn: &Connection) -> StoreResult<i64> {
    let max: Option<i64> =
        db(conn.query_row("SELECT MAX(position) FROM saved_view", [], |row| row.get(0)))?;
    // Table vide → 0, sinon débordement refusé avant l'INSERT.
    max.map_or(Ok(0), |position| {
        position
            .checked_add(1)
            .ok_or_else(|| StoreError::Invalid("position overflow".to_string()))
    })
}

impl ViewStore for SqliteStore {
    fn list_views(&self) -> StoreResult<Vec<SavedView>> {
        self.with_conn(|conn| {
            let sql = format!("SELECT {VIEW_COLUMNS} FROM saved_view ORDER BY position, id");
            let mut statement = db(conn.prepare(&sql))?;
            let raws = collect_rows(&mut statement, [], raw_view_from_row)?;
            raws.into_iter().map(view_from_raw).collect()
        })
    }

    fn get_view(&self, id: ViewId) -> StoreResult<Option<SavedView>> {
        self.with_conn(|conn| view_by_id(conn, id))
    }

    fn get_view_by_name(&self, name: &str) -> StoreResult<Option<SavedView>> {
        self.with_conn(|conn| view_by_name(conn, name))
    }

    fn create_view(&self, view: &NewSavedView) -> StoreResult<SavedView> {
        view.filter.validate()?;
        let name = view_name(&view.name)?;
        let filter_json = encode_filter(&view.filter)?;
        let sort = view.sort.encode();
        let now = ts_to_str(OffsetDateTime::now_utc());
        self.with_conn(|conn| {
            // 0 = ajout en fin, MAX(position)+1 sur une table vide vaut 0.
            let position = if view.position == 0 {
                next_position(conn)?
            } else {
                view.position
            };
            db(conn.execute(
                "INSERT INTO saved_view(name, filter_json, icon, color, position, sort, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
                params![
                    name,
                    filter_json,
                    view.icon,
                    view.color.map(tag_color_to_str),
                    position,
                    sort,
                    now
                ],
            ))?;
            view_by_id(conn, ViewId(conn.last_insert_rowid()))?
                .ok_or_else(|| StoreError::Db("inserted view is missing".to_string()))
        })
    }

    fn update_view(&self, id: ViewId, patch: &SavedViewPatch) -> StoreResult<SavedView> {
        let name = match &patch.name {
            Some(raw) => Some(view_name(raw)?),
            None => None,
        };
        let filter_json = match &patch.filter {
            Some(filter) => {
                filter.validate()?;
                Some(encode_filter(filter)?)
            }
            None => None,
        };
        let sort: Option<Sort> = patch.sort;
        self.with_conn(|conn| {
            let mut sets: Vec<String> = Vec::new();
            let mut values: Vec<SqlValue> = Vec::new();
            if let Some(name) = name {
                set(&mut sets, &mut values, "name", SqlValue::Text(name));
            }
            if let Some(filter_json) = filter_json {
                set(
                    &mut sets,
                    &mut values,
                    "filter_json",
                    SqlValue::Text(filter_json),
                );
            }
            if let Some(icon) = &patch.icon {
                let value = icon
                    .as_ref()
                    .map(|icon| SqlValue::Text(icon.clone()))
                    .unwrap_or(SqlValue::Null);
                set(&mut sets, &mut values, "icon", value);
            }
            if let Some(color) = &patch.color {
                let value = color
                    .map(|color| SqlValue::Text(tag_color_to_str(color).to_string()))
                    .unwrap_or(SqlValue::Null);
                set(&mut sets, &mut values, "color", value);
            }
            if let Some(position) = patch.position {
                set(
                    &mut sets,
                    &mut values,
                    "position",
                    SqlValue::Integer(position),
                );
            }
            if let Some(sort) = sort {
                set(
                    &mut sets,
                    &mut values,
                    "sort",
                    SqlValue::Text(sort.encode()),
                );
            }
            set(
                &mut sets,
                &mut values,
                "updated_at",
                SqlValue::Text(ts_to_str(OffsetDateTime::now_utc())),
            );
            values.push(SqlValue::Integer(id.0));
            let sql = format!("UPDATE saved_view SET {} WHERE id = ?", sets.join(", "));
            let changed = db(conn.execute(&sql, params_from_iter(values.iter())))?;
            if changed == 0 {
                return Err(StoreError::NotFound("view"));
            }
            view_by_id(conn, id)?.ok_or(StoreError::NotFound("view"))
        })
    }

    fn delete_view(&self, id: ViewId) -> StoreResult<bool> {
        self.with_conn(|conn| delete_by_id(conn, "saved_view", id.0))
    }
}

#[cfg(test)]
mod tests;
