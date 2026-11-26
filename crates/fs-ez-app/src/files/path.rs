use std::path::{Path, PathBuf};

use fs_ez_fs::display_lossy;

use crate::app::App;
use crate::error::AppError;

impl App {
    /// Parent confiné sans suivre le dernier composant, refuse le dossier d'état.
    pub(super) fn confine_entry_path(&self, path: &Path) -> Result<PathBuf, AppError> {
        let confined = self.root.confine_new(path)?;
        self.ensure_outside_state(&confined)?;
        Ok(confined)
    }

    /// Refuse un chemin confiné situé dans le dossier d'état fs-ez.
    pub(crate) fn ensure_outside_state(&self, path: &Path) -> Result<(), AppError> {
        if path.starts_with(self.root.state_dir()) {
            return Err(AppError::Invalid(format!(
                "path is inside the state directory: {}",
                display_lossy(path)
            )));
        }
        Ok(())
    }
}
