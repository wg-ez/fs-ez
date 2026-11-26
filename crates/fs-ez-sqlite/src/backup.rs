//! Copie cohérente de la base SQLite via `VACUUM INTO`.

use std::path::Path;

use fs_ez_store::{BackupStore, StoreResult};

use crate::store::{SqliteStore, map_db};

impl BackupStore for SqliteStore {
    fn backup_to(&self, target: &Path) -> StoreResult<()> {
        let target = target.to_string_lossy();
        self.with_conn(|conn| {
            conn.execute("VACUUM INTO ?1", [target.as_ref()])
                .map_err(map_db)?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests;
