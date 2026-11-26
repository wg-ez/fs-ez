//! Implementation de `AuthStore` sur SQLite.

use fs_ez_core::{ApiTokenId, SessionId, UserId, has_unsafe_text};
use fs_ez_store::{
    ApiTokenRecord, AuthStore, STALE_CREDENTIAL_GENERATION, SessionRecord, StoreError, StoreResult,
    UserRecord, normalize_api_token_name,
};
use rusqlite::{Connection, Row, params};
use time::OffsetDateTime;

use crate::convert::{opt_ts_column, row_count, ts_column, ts_to_str};
use crate::store::{SqliteStore, collect_rows, db, delete_by_id, query_optional, with_immediate};

const MAX_USERNAME_CHARS: usize = 64;
const TOKEN_HASH_HEX_CHARS: usize = 64;

const USER_COLUMNS: &str =
    "id, username, password_hash, created_at, updated_at, credential_generation";
const SESSION_COLUMNS: &str =
    "id, token_hash, created_at, expires_at, last_seen_at, credential_generation";
const API_TOKEN_COLUMNS: &str =
    "id, name, token_hash, created_at, expires_at, last_used_at, credential_generation";

fn user_from_row(row: &Row<'_>) -> rusqlite::Result<UserRecord> {
    Ok(UserRecord {
        id: UserId(row.get(0)?),
        username: row.get(1)?,
        password_hash: row.get(2)?,
        created_at: ts_column(row, 3)?,
        updated_at: ts_column(row, 4)?,
        credential_generation: row.get(5)?,
    })
}

fn session_from_row(row: &Row<'_>) -> rusqlite::Result<SessionRecord> {
    Ok(SessionRecord {
        id: SessionId(row.get(0)?),
        token_hash: row.get(1)?,
        created_at: ts_column(row, 2)?,
        expires_at: ts_column(row, 3)?,
        last_seen_at: ts_column(row, 4)?,
        credential_generation: row.get(5)?,
    })
}

fn api_token_from_row(row: &Row<'_>) -> rusqlite::Result<ApiTokenRecord> {
    Ok(ApiTokenRecord {
        id: ApiTokenId(row.get(0)?),
        name: row.get(1)?,
        token_hash: row.get(2)?,
        created_at: ts_column(row, 3)?,
        expires_at: opt_ts_column(row, 4)?,
        last_used_at: opt_ts_column(row, 5)?,
        credential_generation: row.get(6)?,
    })
}

fn validate_username(raw: &str) -> StoreResult<String> {
    let username = raw.trim();
    if username.is_empty() || username.chars().count() > MAX_USERNAME_CHARS || has_unsafe_text(raw)
    {
        return Err(StoreError::Invalid(format!("invalid username {raw:?}")));
    }
    Ok(username.to_string())
}

fn validate_token_name(raw: &str) -> StoreResult<String> {
    normalize_api_token_name(raw)
        .ok_or_else(|| StoreError::Invalid(format!("invalid token name {raw:?}")))
}

/// SHA-256 hexadecimal, 64 caracteres, refuse tout hash arbitraire avant insertion.
fn validate_token_hash(raw: &str) -> StoreResult<()> {
    if raw.len() != TOKEN_HASH_HEX_CHARS || !raw.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(StoreError::Invalid(
            "token hash must be 64 hexadecimal characters".to_string(),
        ));
    }
    Ok(())
}

// Un hash invalide ne peut correspondre a aucune ligne, on evite une erreur sur un cookie corrompu.
fn validate_lookup_hash(raw: &str) -> Option<&str> {
    validate_token_hash(raw).ok().map(|()| raw)
}

fn user_row(conn: &Connection) -> StoreResult<Option<UserRecord>> {
    let sql = format!("SELECT {USER_COLUMNS} FROM user WHERE id = 1");
    query_optional(conn, &sql, [], user_from_row)
}

fn session_by_hash(conn: &Connection, token_hash: &str) -> StoreResult<Option<SessionRecord>> {
    let sql = format!("SELECT {SESSION_COLUMNS} FROM session WHERE token_hash = ?1");
    query_optional(conn, &sql, [token_hash], session_from_row)
}

fn api_token_by_id(conn: &Connection, id: ApiTokenId) -> StoreResult<Option<ApiTokenRecord>> {
    let sql = format!("SELECT {API_TOKEN_COLUMNS} FROM api_token WHERE id = ?1");
    query_optional(conn, &sql, [id.0], api_token_from_row)
}

fn api_token_by_hash(conn: &Connection, token_hash: &str) -> StoreResult<Option<ApiTokenRecord>> {
    let sql = format!("SELECT {API_TOKEN_COLUMNS} FROM api_token WHERE token_hash = ?1");
    query_optional(conn, &sql, [token_hash], api_token_from_row)
}

// Table et colonne viennent de constantes internes, mise a jour best-effort sans erreur si la ligne a disparu.
fn touch_timestamp(
    conn: &Connection,
    table: &'static str,
    column: &'static str,
    id: i64,
    at: &str,
) -> StoreResult<()> {
    let sql = format!("UPDATE {table} SET {column} = ?1 WHERE id = ?2");
    db(conn.execute(&sql, params![at, id]))?;
    Ok(())
}

impl AuthStore for SqliteStore {
    fn get_user(&self) -> StoreResult<Option<UserRecord>> {
        self.with_conn(user_row)
    }

    fn upsert_user(
        &self,
        username: &str,
        password_hash: &str,
        at: OffsetDateTime,
    ) -> StoreResult<UserRecord> {
        let username = validate_username(username)?;
        let at = ts_to_str(at);
        self.with_conn(|conn| {
            // ID fixe a 1 : insertion unique, une ligne deja presente interdit tout ecrasement des identifiants.
            let inserted = db(conn.execute(
                "INSERT INTO user(id, username, password_hash, created_at, updated_at)
                 VALUES (1, ?1, ?2, ?3, ?3)
                 ON CONFLICT(id) DO NOTHING",
                params![username, password_hash, at],
            ))?;
            if inserted == 0 {
                return Err(StoreError::Conflict("a user already exists".to_string()));
            }
            user_row(conn)?
                .ok_or_else(|| StoreError::Db("user row missing after insert".to_string()))
        })
    }

    fn reset_user_credentials(
        &self,
        username: &str,
        password_hash: &str,
        at: OffsetDateTime,
    ) -> StoreResult<UserRecord> {
        let username = validate_username(username)?;
        let at = ts_to_str(at);
        self.with_conn_mut(|conn| {
            // BEGIN IMMEDIATE prend le verrou d'ecriture avant la lecture, aucun login ne s'intercale entre le reset et les suppressions.
            with_immediate(conn, |transaction| {
                db(transaction.execute(
                    "INSERT INTO user(id, username, password_hash, created_at, updated_at)
                     VALUES (1, ?1, ?2, ?3, ?3)
                     ON CONFLICT(id) DO UPDATE SET username = excluded.username,
                     password_hash = excluded.password_hash, updated_at = excluded.updated_at,
                     credential_generation = credential_generation + 1",
                    params![username, password_hash, at],
                ))?;
                db(transaction.execute("DELETE FROM session", []))?;
                db(transaction.execute("DELETE FROM api_token", []))?;
                user_row(transaction)?
                    .ok_or_else(|| StoreError::Db("user row missing after reset".to_string()))
            })
        })
    }

    fn create_session(
        &self,
        token_hash: &str,
        credential_generation: i64,
        created_at: OffsetDateTime,
        expires_at: OffsetDateTime,
    ) -> StoreResult<SessionId> {
        validate_token_hash(token_hash)?;
        let created_at = ts_to_str(created_at);
        let expires_at = ts_to_str(expires_at);
        self.with_conn(|conn| {
            db(conn.execute(
                "INSERT INTO session(token_hash, created_at, expires_at, last_seen_at, credential_generation)
                 VALUES (?1, ?2, ?3, ?2, ?4)",
                params![token_hash, created_at, expires_at, credential_generation],
            ))?;
            Ok(SessionId(conn.last_insert_rowid()))
        })
    }

    fn find_session(&self, token_hash: &str) -> StoreResult<Option<SessionRecord>> {
        let Some(token_hash) = validate_lookup_hash(token_hash) else {
            return Ok(None);
        };
        self.with_conn(|conn| session_by_hash(conn, token_hash))
    }

    fn touch_session(&self, id: SessionId, at: OffsetDateTime) -> StoreResult<()> {
        let at = ts_to_str(at);
        self.with_conn(|conn| touch_timestamp(conn, "session", "last_seen_at", id.0, &at))
    }

    fn delete_session(&self, id: SessionId) -> StoreResult<bool> {
        self.with_conn(|conn| delete_by_id(conn, "session", id.0))
    }

    fn delete_session_by_hash(&self, token_hash: &str) -> StoreResult<bool> {
        let Some(token_hash) = validate_lookup_hash(token_hash) else {
            return Ok(false);
        };
        self.with_conn(|conn| {
            let changed =
                db(conn.execute("DELETE FROM session WHERE token_hash = ?1", [token_hash]))?;
            Ok(changed > 0)
        })
    }

    fn delete_all_sessions(&self) -> StoreResult<u64> {
        self.with_conn(|conn| row_count(db(conn.execute("DELETE FROM session", []))?))
    }

    fn purge_expired_sessions(&self, now: OffsetDateTime) -> StoreResult<u64> {
        let now = ts_to_str(now);
        self.with_conn(|conn| {
            // Comparaison TEXT sure car tous les timestamps sont ecrits UTC avec le meme format RFC3339.
            row_count(db(
                conn.execute("DELETE FROM session WHERE expires_at < ?1", [now])
            )?)
        })
    }

    fn create_api_token(
        &self,
        name: &str,
        token_hash: &str,
        credential_generation: i64,
        created_at: OffsetDateTime,
        expires_at: Option<OffsetDateTime>,
    ) -> StoreResult<ApiTokenRecord> {
        let name = validate_token_name(name)?;
        validate_token_hash(token_hash)?;
        let created_at = ts_to_str(created_at);
        let expires_at = expires_at.map(ts_to_str);
        self.with_conn_mut(|conn| {
            // Relecture et insertion sous le meme verrou immediat, un reset ne peut pas s'intercaler.
            with_immediate(conn, |transaction| {
                let current = user_row(transaction)?.map_or(0, |user| user.credential_generation);
                if current != credential_generation {
                    return Err(StoreError::Conflict(
                        STALE_CREDENTIAL_GENERATION.to_string(),
                    ));
                }
                db(transaction.execute(
                    "INSERT INTO api_token(name, token_hash, created_at, expires_at, credential_generation)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![name, token_hash, created_at, expires_at, credential_generation],
                ))?;
                api_token_by_id(transaction, ApiTokenId(transaction.last_insert_rowid()))?
                    .ok_or_else(|| StoreError::Db("inserted api token is missing".to_string()))
            })
        })
    }

    fn list_api_tokens(&self) -> StoreResult<Vec<ApiTokenRecord>> {
        self.with_conn(|conn| {
            let sql = format!(
                "SELECT {API_TOKEN_COLUMNS} FROM api_token ORDER BY created_at DESC, id DESC"
            );
            let mut statement = db(conn.prepare(&sql))?;
            collect_rows(&mut statement, [], api_token_from_row)
        })
    }

    fn find_api_token_by_hash(&self, token_hash: &str) -> StoreResult<Option<ApiTokenRecord>> {
        let Some(token_hash) = validate_lookup_hash(token_hash) else {
            return Ok(None);
        };
        self.with_conn(|conn| api_token_by_hash(conn, token_hash))
    }

    fn touch_api_token(&self, id: ApiTokenId, at: OffsetDateTime) -> StoreResult<()> {
        let at = ts_to_str(at);
        self.with_conn(|conn| touch_timestamp(conn, "api_token", "last_used_at", id.0, &at))
    }

    fn revoke_api_token(&self, id: ApiTokenId) -> StoreResult<bool> {
        self.with_conn(|conn| delete_by_id(conn, "api_token", id.0))
    }

    fn delete_all_api_tokens(&self) -> StoreResult<u64> {
        self.with_conn(|conn| row_count(db(conn.execute("DELETE FROM api_token", []))?))
    }
}

#[cfg(test)]
mod tests;
