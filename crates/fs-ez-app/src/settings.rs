//! Accès aux réglages persistés, validés par le domaine avant écriture.

use crate::app::App;
use crate::error::AppError;

impl App {
    pub fn get_setting(&self, key: &str) -> Result<Option<String>, AppError> {
        Ok(self.store.get_setting(key)?)
    }

    /// Valide sur une copie complète : une clé inconnue ou une valeur invalide n'écrit rien.
    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), AppError> {
        let mut settings = self.settings()?;
        settings.set(key, value)?;
        self.store.set_setting(key, value)?;
        Ok(())
    }

    pub fn all_settings(&self) -> Result<Vec<(String, String)>, AppError> {
        Ok(self.store.all_settings()?)
    }
}
