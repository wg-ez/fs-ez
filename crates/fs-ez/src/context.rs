//! Construction du contexte CLI : root, base SQLite et `App`.

use std::path::PathBuf;
use std::sync::Arc;

use fs_ez_app::{App, FileConfig, first_some, load_file_config};
use fs_ez_cli::context::CliContext;
use fs_ez_cli::{CliError, GlobalOptions};
use fs_ez_fingerprint::Blake3Fingerprinter;
use fs_ez_fs::{Root, SystemFs};
use fs_ez_sqlite::SqliteStore;
use fs_ez_store::Store;

pub fn build(globals: &GlobalOptions) -> Result<CliContext, CliError> {
    let root =
        Root::discover(globals.root.as_deref()).map_err(|error| CliError::Io(error.to_string()))?;
    let config = load_file_config(&root)?;
    let db_path = resolve_db(globals, &config, &root);
    let store = SqliteStore::open(&db_path).map_err(|error| CliError::Io(error.to_string()))?;
    let app = App::new(
        Arc::new(store) as Arc<dyn Store>,
        Arc::new(SystemFs),
        Arc::new(Blake3Fingerprinter::default()),
        root,
    )
    .with_db_path(db_path.clone());
    Ok(CliContext {
        app,
        globals: globals.clone(),
        db_path,
    })
}

/// Priorité flags CLI > `fs-ez.toml` > défaut du root.
fn resolve_db(globals: &GlobalOptions, config: &FileConfig, root: &Root) -> PathBuf {
    first_some([globals.db.clone(), config.db.clone()], || {
        root.default_db()
    })
}
