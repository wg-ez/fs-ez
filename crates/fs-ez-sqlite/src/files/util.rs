//! Utilitaires partages par les operations `FileStore`.

use fs_ez_store::StoreResult;
use rusqlite::{Connection, Params};
use time::{Duration, OffsetDateTime};

use crate::convert::{to_count, ts_to_str};
use crate::store::db;

/// Marge sous la limite de variables SQLite (999 historiquement, 32766 en moderne).
pub(super) const PARAM_BATCH: usize = 900;

/// Taille de tranche pour un `IN` qui consomme déjà `extra` paramètres fixes.
pub(super) fn chunk_len(extra: usize) -> usize {
    PARAM_BATCH - extra
}

pub(super) fn placeholders(count: usize) -> String {
    let mut sql = String::with_capacity(count.saturating_mul(2));
    for index in 0..count {
        if index > 0 {
            sql.push(',');
        }
        sql.push('?');
    }
    sql
}

pub(super) fn escape_like(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        if matches!(character, '\\' | '%' | '_') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

pub(super) fn cutoff(days: i64) -> String {
    let seconds = days.saturating_mul(86_400);
    OffsetDateTime::now_utc()
        .checked_sub(Duration::seconds(seconds))
        .map(ts_to_str)
        .unwrap_or_else(|| ts_to_str(OffsetDateTime::UNIX_EPOCH))
}

/// `query_row` d'une ligne scalaire, erreurs et conversion via `to_count`.
pub(super) fn count(conn: &Connection, sql: &str, params: impl Params) -> StoreResult<u64> {
    let value: i64 = db(conn.query_row(sql, params, |row| row.get(0)))?;
    to_count(value)
}
