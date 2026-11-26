//! Store SQLite : une connexion derriere un mutex, partagee par les ports.

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use fs_ez_fs::PinnedDir;
use fs_ez_store::{StoreError, StoreResult};
use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior};

use crate::SqliteError;
use crate::db;

pub struct SqliteStore {
    conn: Mutex<Connection>,
    // Parent epingle vivant : `-wal`/`-shm` restent dans l'inode ouvert, jamais re-resolus.
    _parent: Option<PinnedDir>,
}

impl SqliteStore {
    pub fn open(path: &Path) -> Result<Self, SqliteError> {
        let opened = db::open_pinned(path)?;
        Ok(Self {
            conn: Mutex::new(opened.conn),
            _parent: opened.parent,
        })
    }

    pub fn from_connection(conn: Connection) -> Self {
        Self {
            conn: Mutex::new(conn),
            _parent: None,
        }
    }

    pub fn schema_version(path: &Path) -> Result<i64, SqliteError> {
        let conn = db::open(path)?;
        db::schema_version(&conn)
    }

    // Les methodes publiques se verrouillent ici une seule fois, les fonctions internes prennent `&Connection`.
    pub(crate) fn with_conn<T>(
        &self,
        f: impl FnOnce(&Connection) -> StoreResult<T>,
    ) -> StoreResult<T> {
        let guard = self.lock()?;
        f(&guard)
    }

    pub(crate) fn with_conn_mut<T>(
        &self,
        f: impl FnOnce(&mut Connection) -> StoreResult<T>,
    ) -> StoreResult<T> {
        let mut guard = self.lock()?;
        f(&mut guard)
    }

    fn lock(&self) -> StoreResult<MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|_| StoreError::Db("connection mutex poisoned".to_string()))
    }
}

/// Une transaction `BEGIN IMMEDIATE`, commit si le corps réussit, rollback implicite au Drop sinon.
pub(crate) fn with_immediate<T>(
    conn: &mut Connection,
    f: impl FnOnce(&Transaction<'_>) -> StoreResult<T>,
) -> StoreResult<T> {
    let transaction = db(conn.transaction_with_behavior(TransactionBehavior::Immediate))?;
    let value = f(&transaction)?;
    db(transaction.commit())?;
    Ok(value)
}

pub(crate) fn set(
    sets: &mut Vec<String>,
    values: &mut Vec<SqlValue>,
    column: &str,
    value: SqlValue,
) {
    sets.push(format!("{column} = ?"));
    values.push(value);
}

pub(crate) fn db<T>(result: rusqlite::Result<T>) -> StoreResult<T> {
    result.map_err(map_db)
}

/// `query_row` + `optional`, une ligne absente donne `Ok(None)`, les erreurs passent par `db`.
pub(crate) fn query_optional<T, P, F>(
    conn: &Connection,
    sql: &str,
    params: P,
    map: F,
) -> StoreResult<Option<T>>
where
    P: rusqlite::Params,
    F: FnOnce(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
{
    db(conn.query_row(sql, params, map).optional())
}

/// Prepare + `query_map` + collecte, les erreurs de ligne remontent via `db`.
pub(crate) fn collect_rows<T, P, F>(
    statement: &mut rusqlite::Statement<'_>,
    params: P,
    map: F,
) -> StoreResult<Vec<T>>
where
    P: rusqlite::Params,
    F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
{
    let rows = db(statement.query_map(params, map))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(db(row)?);
    }
    Ok(out)
}

// La table vient d'une constante interne, seul l'identifiant passe en parametre.
pub(crate) fn delete_by_id(conn: &Connection, table: &'static str, id: i64) -> StoreResult<bool> {
    let sql = format!("DELETE FROM {table} WHERE id = ?1");
    let changed = db(conn.execute(&sql, [id]))?;
    Ok(changed > 0)
}

pub(crate) fn map_db(error: rusqlite::Error) -> StoreError {
    match &error {
        rusqlite::Error::SqliteFailure(code, message)
            if code.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            StoreError::Conflict(message.clone().unwrap_or_else(|| error.to_string()))
        }
        _ => StoreError::Db(error.to_string()),
    }
}
