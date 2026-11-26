use super::*;
use std::sync::Arc;

use fs_ez_fingerprint::Blake3Fingerprinter;
use fs_ez_fs::{Root, SystemFs};
use fs_ez_sqlite::{SqliteStore, db};
use fs_ez_store::Store;
use tempfile::TempDir;

fn test_app(temp: &TempDir) -> App {
    let store = Arc::new(SqliteStore::from_connection(
        db::open_in_memory().expect("base mémoire"),
    )) as Arc<dyn Store>;
    App::new(
        store,
        Arc::new(SystemFs),
        Arc::new(Blake3Fingerprinter::default()),
        Root::discover(Some(temp.path())).expect("root"),
    )
}

#[test]
fn without_env_password_the_web_keeps_onboarding_open() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    assert!(!app.has_user().expect("has_user"));

    assert!(!create_env_user(&app, Some("user"), None).expect("bootstrap"));
    assert!(!app.has_user().expect("has_user"));
}

#[test]
fn password_without_username_is_refused() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    let password = "mot-de-passe-de-test";
    for username in [None, Some("   ")] {
        assert!(
            create_env_user(&app, username, Some(password)).is_err(),
            "nom d'utilisateur manquant accepté: {username:?}"
        );
        assert!(!app.has_user().expect("has_user"));
    }
}

#[test]
fn provided_password_is_used_and_not_stored_in_clear() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    let password = "mot-de-passe-de-test";
    assert!(create_env_user(&app, Some("root"), Some(password)).expect("bootstrap"));
    let user = app.store.get_user().expect("user").expect("présent");
    assert_eq!(user.username, "root");
    assert!(user.password_hash.starts_with("$argon2"));
    assert!(!user.password_hash.contains(password));
    assert!(app.verify_user("root", password).expect("verify"));

    assert!(!create_env_user(&app, Some("root"), Some(password)).expect("idempotent"));
}

#[test]
fn init_creates_files_and_inbox_layout() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    let db_path = temp.path().join("config/fs-ez.db");

    let report = app.init(&db_path).expect("init");

    assert_eq!(report.files, temp.path().join("files"));
    assert!(report.files.is_dir());
    assert!(app.inbox_path().expect("inbox").is_dir());
}

#[test]
fn next_backup_hour_rolls_to_the_next_day_after_the_target() {
    assert_eq!(seconds_until_hour(0, 3, 0), 10_800);
    assert_eq!(seconds_until_hour(3 * 3_600, 3, 0), 86_400);
    assert_eq!(seconds_until_hour(4 * 3_600, 3, 0), 82_800);
    assert_eq!(seconds_until_hour(0, 0, 0), 86_400);
    assert_eq!(seconds_until_hour(23 * 3_600 - 1, 23, 0), 1);
}

#[test]
fn next_backup_hour_uses_the_local_offset() {
    assert_eq!(seconds_until_hour(0, 3, 3_600), 7_200);
    assert_eq!(seconds_until_hour(4 * 3_600, 3, 3_600), 79_200);
    assert_eq!(seconds_until_hour(0, 3, -7_200), 18_000);
    assert_eq!(seconds_until_hour(0, 0, -7_200), 7_200);
}
