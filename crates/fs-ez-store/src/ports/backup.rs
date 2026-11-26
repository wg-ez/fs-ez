//! Port de copie cohérente de la base.

use std::path::Path;

use crate::error::StoreResult;

pub trait BackupStore {
    /// Écrit une copie cohérente de la base, la cible ne doit pas exister.
    fn backup_to(&self, target: &Path) -> StoreResult<()>;
}
