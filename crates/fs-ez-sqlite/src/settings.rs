//! Implementation de `SettingsStore` sur SQLite.

use fs_ez_store::{SettingsStore, StoreError, StoreResult};

use crate::store::{SqliteStore, collect_rows, db, query_optional};

const MAX_SETTING_KEY_CHARS: usize = 100;
const MAX_SETTING_VALUE_CHARS: usize = 4096;

fn validate_key(key: &str) -> StoreResult<()> {
    if key.trim().is_empty() || key.chars().count() > MAX_SETTING_KEY_CHARS {
        return Err(StoreError::Invalid(format!("invalid setting key {key:?}")));
    }
    Ok(())
}

fn validate_value(value: &str) -> StoreResult<()> {
    if value.chars().count() > MAX_SETTING_VALUE_CHARS {
        return Err(StoreError::Invalid(
            "setting value exceeds 4096 characters".to_string(),
        ));
    }
    Ok(())
}

impl SettingsStore for SqliteStore {
    fn get_setting(&self, key: &str) -> StoreResult<Option<String>> {
        self.with_conn(|conn| {
            query_optional(
                conn,
                "SELECT value FROM setting WHERE key = ?1",
                [key],
                |row| row.get(0),
            )
        })
    }

    fn set_setting(&self, key: &str, value: &str) -> StoreResult<()> {
        validate_key(key)?;
        validate_value(value)?;
        self.with_conn(|conn| {
            db(conn.execute(
                "INSERT INTO setting(key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [key, value],
            ))?;
            Ok(())
        })
    }

    fn delete_setting(&self, key: &str) -> StoreResult<bool> {
        self.with_conn(|conn| {
            let changed = db(conn.execute("DELETE FROM setting WHERE key = ?1", [key]))?;
            Ok(changed > 0)
        })
    }

    fn all_settings(&self) -> StoreResult<Vec<(String, String)>> {
        self.with_conn(|conn| {
            let mut statement = db(conn.prepare("SELECT key, value FROM setting ORDER BY key"))?;
            collect_rows(&mut statement, [], |row| Ok((row.get(0)?, row.get(1)?)))
        })
    }
}

#[cfg(test)]
mod tests;
