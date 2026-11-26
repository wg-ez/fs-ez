use std::sync::Arc;

use fs_ez_fingerprint::Blake3Fingerprinter;
use fs_ez_fs::{Root, SystemFs};
use fs_ez_sqlite::db;
use fs_ez_store::Store;
use tempfile::TempDir;

use super::*;

fn context(root: &TempDir) -> CliContext {
    let store = Arc::new(fs_ez_sqlite::SqliteStore::from_connection(
        db::open_in_memory().expect("base mémoire"),
    )) as Arc<dyn Store>;
    let app = fs_ez_app::App::new(
        store,
        Arc::new(SystemFs),
        Arc::new(Blake3Fingerprinter::default()),
        Root::discover(Some(root.path())).expect("root"),
    );
    CliContext {
        app,
        globals: Default::default(),
        db_path: root.path().join("fs-ez.db"),
    }
}

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

#[test]
fn serve_outcome_keeps_only_cli_listen_flags() {
    let temp = TempDir::new().expect("tempdir");
    std::fs::write(
        temp.path().join("fs-ez.toml"),
        "host = \"0.0.0.0\"\nport = 18991\n",
    )
    .expect("écriture");
    let ctx = context(&temp);

    let parsed = crate::parse(&args(&["serve"])).expect("parse");
    match execute(&parsed, &ctx).expect("exécution") {
        Outcome::Serve { host, port } => {
            assert_eq!(host, None);
            assert_eq!(port, None);
        }
        _ => panic!("attendu Serve"),
    }

    let parsed =
        crate::parse(&args(&["serve", "--host", "127.0.0.1", "--port", "19000"])).expect("parse");
    match execute(&parsed, &ctx).expect("exécution") {
        Outcome::Serve { host, port } => {
            assert_eq!(host.as_deref(), Some("127.0.0.1"));
            assert_eq!(port, Some(19000));
        }
        _ => panic!("attendu Serve"),
    }
}
