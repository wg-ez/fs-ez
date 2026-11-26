//! Constructeurs d'état partagés par les tests unitaires de `fs-ez-web`.

use std::path::PathBuf;
use std::sync::Arc;

use fs_ez_app::App;
use fs_ez_fingerprint::Blake3Fingerprinter;
use fs_ez_fs::{Root, SystemFs};
use fs_ez_sqlite::{SqliteStore, db};
use fs_ez_store::Store;
use tempfile::TempDir;

use crate::backend::AppBackend;
use crate::config::WebConfig;
use crate::locales::Locales;
use crate::state::AppState;

pub(crate) const USERNAME: &str = "user";
pub(crate) const PASSWORD: &str = "correct-horse-battery-staple";

/// État de test standard, store mémoire + utilisateur.
pub(crate) fn state(temp: &TempDir) -> AppState<AppBackend> {
    let store = Arc::new(SqliteStore::from_connection(
        db::open_in_memory().expect("base mémoire"),
    )) as Arc<dyn Store>;
    build(temp, store, true, temp.path().join("config/fs-ez.db"))
}

/// Store persistant sans utilisateur, requis pour observer une écriture externe.
pub(crate) fn persistent_state(temp: &TempDir) -> AppState<AppBackend> {
    let db_path = temp.path().join("fs-ez.db");
    let store = Arc::new(SqliteStore::open(&db_path).expect("store")) as Arc<dyn Store>;
    build(temp, store, false, db_path)
}

fn build(
    temp: &TempDir,
    store: Arc<dyn Store>,
    user: bool,
    db_path: PathBuf,
) -> AppState<AppBackend> {
    let app = App::new(
        store,
        Arc::new(SystemFs),
        Arc::new(Blake3Fingerprinter::default()),
        Root::discover(Some(temp.path())).expect("root"),
    );
    if user {
        app.init_user(USERNAME, PASSWORD).expect("user");
    }
    let config = WebConfig {
        host: "127.0.0.1".to_string(),
        port: 8678,
        max_upload: 1024,
        root: temp.path().to_path_buf(),
        db_path,
        files: None,
    };
    AppState::new(
        Arc::new(AppBackend::new(Arc::new(app))),
        Arc::new(config),
        Arc::new(Locales::load().expect("locales")),
    )
}
