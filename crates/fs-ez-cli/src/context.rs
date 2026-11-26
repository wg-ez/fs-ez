//! Contexte d'exécution CLI, sans dépendance d'infrastructure.

use std::path::PathBuf;

use fs_ez_app::App;

use crate::globals::GlobalOptions;

#[derive(Clone)]
pub struct CliContext {
    pub app: App,
    pub globals: GlobalOptions,
    pub db_path: PathBuf,
}
