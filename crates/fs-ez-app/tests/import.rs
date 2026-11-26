//! Porte phase 4 côté domaine : doublons, ré-import, scan et fichiers intacts.

use fs_ez_app::{ImportCounts, LibraryOutcome, sharded_path};
use std::ffi::OsStr;
use std::os::unix::fs::MetadataExt;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, UNIX_EPOCH};

mod common;

use common::{
    Harness, ScriptedFs, app_with_fingerprinter, app_with_fs, app_with_trigger_sql, files_under,
    import_counts, modified, set_modified, stored_path, write,
};
use fs_ez_app::App;
use tempfile::TempDir;

fn scripted_app(temp: &TempDir) -> (fs_ez_app::App, Arc<ScriptedFs>) {
    let fs = Arc::new(ScriptedFs::new());
    let app = app_with_fs(temp, fs.clone());
    (app, fs)
}

fn scripted_setup() -> (TempDir, fs_ez_app::App, Arc<ScriptedFs>, std::path::PathBuf) {
    let temp = TempDir::new().expect("racine");
    std::fs::create_dir_all(temp.path().join("files")).expect("files");
    let (app, fs) = scripted_app(&temp);
    let inbox = temp.path().join("inbox");
    std::fs::create_dir_all(&inbox).expect("inbox");
    (temp, app, fs, inbox)
}

fn known(app: &fs_ez_app::App, name: &str) -> bool {
    app.find_by_name(name).expect("base").is_some()
}

fn missing(app: &fs_ez_app::App, name: &str) -> bool {
    app.find_by_name(name).expect("base").is_none()
}

fn app_with_failing_inbox_trigger(temp: &TempDir) -> App {
    std::fs::create_dir_all(temp.path().join("files")).expect("files");
    app_with_trigger_sql(
        temp,
        "CREATE TRIGGER inbox_boom BEFORE INSERT ON file WHEN NEW.name = 'boom.txt'
         BEGIN SELECT RAISE(ABORT, 'boom'); END;",
    )
}

#[path = "import/inbox_batch.rs"]
mod inbox_batch;
#[path = "import/inbox_placement.rs"]
mod inbox_placement;
#[path = "import/inbox_walk.rs"]
mod inbox_walk;
#[path = "import/scan.rs"]
mod scan;
