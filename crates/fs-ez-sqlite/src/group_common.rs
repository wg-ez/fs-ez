//! Mecanique commune aux groupes de tags et de vues, les traits publics restent separes.

use fs_ez_core::{TagColor, normalize_group_name};
use fs_ez_store::{StoreError, StoreResult};
use rusqlite::types::{Type, Value as SqlValue};
use rusqlite::{Connection, Row, params, params_from_iter};

use crate::convert::{conversion, position_from_index, tag_color_from_str, tag_color_to_str};
use crate::store::{collect_rows, db, delete_by_id, query_optional, set};

/// Colonnes communes, l'ordre correspond au convertisseur `group_from_row`.
const GROUP_COLUMNS: &str = "id, name, color, position, collapsed";

/// Champs modifies par `update_group`, `None` laisse la colonne inchangee.
pub(crate) struct GroupUpdate {
    pub(crate) name: Option<String>,
    pub(crate) color: Option<Option<TagColor>>,
    pub(crate) position: Option<i64>,
    pub(crate) collapsed: Option<bool>,
}

/// Noms de tables et messages propres a chaque type de groupe.
pub(crate) struct GroupSpec {
    pub(crate) table: &'static str,
    pub(crate) member_table: &'static str,
    pub(crate) member_column: &'static str,
    pub(crate) missing_after_insert: &'static str,
    pub(crate) not_found: &'static str,
}

pub(crate) fn validate_group_name(raw: &str, label: &str) -> StoreResult<String> {
    normalize_group_name(raw).ok_or_else(|| StoreError::Invalid(format!("invalid {label} {raw:?}")))
}

pub(crate) fn group_from_row<G>(
    row: &Row<'_>,
    make: impl FnOnce(i64, String, Option<TagColor>, i64, bool) -> G,
) -> rusqlite::Result<G> {
    let id: i64 = row.get(0)?;
    let name: String = row.get(1)?;
    let color: Option<String> = row.get(2)?;
    let position: i64 = row.get(3)?;
    let collapsed: i64 = row.get(4)?;
    let color = match color {
        Some(raw) => {
            Some(tag_color_from_str(&raw).map_err(|error| conversion(2, Type::Text, error))?)
        }
        None => None,
    };
    Ok(make(id, name, color, position, collapsed != 0))
}

pub(crate) fn list_groups<G>(
    conn: &Connection,
    table: &str,
    map: impl Fn(&Row<'_>) -> rusqlite::Result<G>,
) -> StoreResult<Vec<G>> {
    let sql = format!("SELECT {GROUP_COLUMNS} FROM {table} ORDER BY position, id");
    let mut statement = db(conn.prepare(&sql))?;
    collect_rows(&mut statement, [], map)
}

pub(crate) fn group_by_id<G>(
    conn: &Connection,
    table: &str,
    id: i64,
    map: impl FnOnce(&Row<'_>) -> rusqlite::Result<G>,
) -> StoreResult<Option<G>> {
    let sql = format!("SELECT {GROUP_COLUMNS} FROM {table} WHERE id = ?1");
    query_optional(conn, &sql, [id], map)
}

pub(crate) fn create_group<G>(
    conn: &mut Connection,
    spec: &GroupSpec,
    name: String,
    color: Option<TagColor>,
    map: impl FnOnce(&Row<'_>) -> rusqlite::Result<G>,
) -> StoreResult<G> {
    let transaction = db(conn.transaction())?;
    let bump = format!("UPDATE {} SET position = position + 1", spec.table);
    db(transaction.execute(&bump, []))?;
    let insert = format!(
        "INSERT INTO {}(name, color, position, collapsed) VALUES (?1, ?2, 0, 0)",
        spec.table
    );
    db(transaction.execute(&insert, params![name, color.map(tag_color_to_str)]))?;
    let group = group_by_id(
        &transaction,
        spec.table,
        transaction.last_insert_rowid(),
        map,
    )?
    .ok_or_else(|| StoreError::Db(spec.missing_after_insert.to_string()))?;
    db(transaction.commit())?;
    Ok(group)
}

pub(crate) fn update_group<G>(
    conn: &Connection,
    spec: &GroupSpec,
    id: i64,
    update: GroupUpdate,
    map: impl Fn(&Row<'_>) -> rusqlite::Result<G>,
) -> StoreResult<G> {
    let mut sets: Vec<String> = Vec::new();
    let mut values: Vec<SqlValue> = Vec::new();
    if let Some(name) = update.name {
        set(&mut sets, &mut values, "name", SqlValue::Text(name));
    }
    if let Some(color) = update.color {
        let value = match color {
            Some(color) => SqlValue::Text(tag_color_to_str(color).to_string()),
            None => SqlValue::Null,
        };
        set(&mut sets, &mut values, "color", value);
    }
    if let Some(position) = update.position {
        set(
            &mut sets,
            &mut values,
            "position",
            SqlValue::Integer(position),
        );
    }
    if let Some(collapsed) = update.collapsed {
        set(
            &mut sets,
            &mut values,
            "collapsed",
            SqlValue::Integer(i64::from(collapsed)),
        );
    }
    if sets.is_empty() {
        return group_by_id(conn, spec.table, id, &map)?
            .ok_or(StoreError::NotFound(spec.not_found));
    }
    values.push(SqlValue::Integer(id));
    let sql = format!("UPDATE {} SET {} WHERE id = ?", spec.table, sets.join(", "));
    let changed = db(conn.execute(&sql, params_from_iter(values.iter())))?;
    if changed == 0 {
        return Err(StoreError::NotFound(spec.not_found));
    }
    group_by_id(conn, spec.table, id, map)?.ok_or(StoreError::NotFound(spec.not_found))
}

pub(crate) fn delete_group(
    conn: &mut Connection,
    table: &'static str,
    cascade_sql: &str,
    id: i64,
) -> StoreResult<bool> {
    let transaction = db(conn.transaction())?;
    db(transaction.execute(cascade_sql, [id]))?;
    let changed = delete_by_id(&transaction, table, id)?;
    db(transaction.commit())?;
    Ok(changed)
}

pub(crate) fn group_for_member(
    conn: &Connection,
    member_table: &str,
    member_column: &str,
    member: i64,
) -> StoreResult<Option<i64>> {
    let sql = format!("SELECT group_id FROM {member_table} WHERE {member_column} = ?1");
    query_optional(conn, &sql, [member], |row| row.get::<_, i64>(0))
}

pub(crate) fn place_member(
    conn: &mut Connection,
    member_table: &str,
    member_column: &str,
    order_sql: &str,
    group: i64,
    member: i64,
    before: Option<i64>,
) -> StoreResult<()> {
    let transaction = db(conn.transaction())?;
    let mut statement = db(transaction.prepare(order_sql))?;
    let mut ids = collect_rows(&mut statement, [group], |row| row.get::<_, i64>(0))?;
    drop(statement);
    ids.retain(|id| *id != member);
    let index = before
        .and_then(|before| ids.iter().position(|id| *id == before))
        .unwrap_or(ids.len());
    ids.insert(index, member);
    let delete_sql = format!("DELETE FROM {member_table} WHERE {member_column} = ?1");
    db(transaction.execute(&delete_sql, [member]))?;
    let insert_sql = format!(
        "INSERT INTO {member_table}(group_id, {member_column}, position) VALUES (?1, ?2, 0)"
    );
    db(transaction.execute(&insert_sql, params![group, member]))?;
    write_member_positions(
        &transaction,
        member_table,
        member_column,
        group,
        ids.iter().copied(),
    )?;
    db(transaction.commit())?;
    Ok(())
}

pub(crate) fn reorder_groups(
    conn: &mut Connection,
    table: &str,
    ids: impl IntoIterator<Item = i64>,
) -> StoreResult<()> {
    let transaction = db(conn.transaction())?;
    write_group_positions(&transaction, table, ids)?;
    db(transaction.commit())?;
    Ok(())
}

pub(crate) fn reorder_members(
    conn: &mut Connection,
    member_table: &str,
    member_column: &str,
    group: i64,
    ids: impl IntoIterator<Item = i64>,
) -> StoreResult<()> {
    let transaction = db(conn.transaction())?;
    write_member_positions(&transaction, member_table, member_column, group, ids)?;
    db(transaction.commit())?;
    Ok(())
}

// Positions 0..n dans l'ordre des ids, `write` applique la requete UPDATE propre a la table.
fn write_positions<I>(ids: I, mut write: impl FnMut(i64, i64) -> StoreResult<()>) -> StoreResult<()>
where
    I: IntoIterator<Item = i64>,
{
    for (index, id) in ids.into_iter().enumerate() {
        write(position_from_index(index)?, id)?;
    }
    Ok(())
}

fn write_group_positions(
    conn: &Connection,
    table: &str,
    ids: impl IntoIterator<Item = i64>,
) -> StoreResult<()> {
    let sql = format!("UPDATE {table} SET position = ?1 WHERE id = ?2");
    let mut statement = db(conn.prepare(&sql))?;
    write_positions(ids, |position, id| {
        db(statement.execute(params![position, id]))?;
        Ok(())
    })
}

fn write_member_positions(
    conn: &Connection,
    member_table: &str,
    member_column: &str,
    group: i64,
    ids: impl IntoIterator<Item = i64>,
) -> StoreResult<()> {
    let sql = format!(
        "UPDATE {member_table} SET position = ?1 WHERE group_id = ?2 AND {member_column} = ?3"
    );
    let mut statement = db(conn.prepare(&sql))?;
    write_positions(ids, |position, id| {
        db(statement.execute(params![position, group, id]))?;
        Ok(())
    })
}
