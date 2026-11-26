//! Port de persistance des réglages clé/valeur.

use crate::error::StoreResult;

pub trait SettingsStore {
    fn get_setting(&self, key: &str) -> StoreResult<Option<String>>;
    fn set_setting(&self, key: &str, value: &str) -> StoreResult<()>;
    fn delete_setting(&self, key: &str) -> StoreResult<bool>;
    fn all_settings(&self) -> StoreResult<Vec<(String, String)>>;
}
