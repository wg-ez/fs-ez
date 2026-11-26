use fs_ez_app::load_file_config;
use tempfile::TempDir;

mod common;

use common::root_of;

#[test]
fn first_some_takes_first_present_in_order_and_falls_back() {
    assert_eq!(fs_ez_app::first_some([None, Some(2), Some(3)], || 0), 2);
    assert_eq!(fs_ez_app::first_some([None, None], || 7), 7);
}

#[test]
fn absent_file_yields_all_none() {
    let temp = TempDir::new().expect("tempdir");
    let config = load_file_config(&root_of(&temp)).expect("absent");
    assert!(config.host.is_none());
    assert!(config.port.is_none());
    assert!(config.max_upload.is_none());
    assert!(config.db.is_none());
    assert!(config.files.is_none());
    assert!(config.uid.is_none());
    assert!(config.gid.is_none());
}

#[test]
fn file_owner_defaults_to_1000_and_reads_toml_ids() {
    let temp = TempDir::new().expect("tempdir");
    let root = root_of(&temp);
    assert_eq!(
        fs_ez_app::file_owner(&root).expect("défaut"),
        fs_ez_app::FileOwner {
            uid: 1000,
            gid: 1000
        }
    );

    std::fs::write(temp.path().join("fs-ez.toml"), "uid = 1001\ngid = 1002\n").expect("écriture");
    assert_eq!(
        fs_ez_app::file_owner(&root).expect("toml"),
        fs_ez_app::FileOwner {
            uid: 1001,
            gid: 1002
        }
    );
}

#[test]
fn valid_file_is_read_and_relative_paths_resolve_under_root() {
    let temp = TempDir::new().expect("tempdir");
    let root = root_of(&temp);
    std::fs::write(
        temp.path().join("fs-ez.toml"),
        "host = \"127.0.0.1\"\nport = 9000\nmax_upload = 4096\ndb = \"data/fs.db\"\nfiles = \"in\"\nuid = 1001\ngid = 1002\n",
    )
    .expect("écriture");

    let config = load_file_config(&root).expect("valide");
    assert_eq!(config.host.as_deref(), Some("127.0.0.1"));
    assert_eq!(config.port, Some(9000));
    assert_eq!(config.max_upload, Some(4096));
    assert_eq!(config.db, Some(root.path().join("data/fs.db")));
    assert_eq!(config.files, Some(root.path().join("in")));
    assert_eq!(config.uid, Some(1001));
    assert_eq!(config.gid, Some(1002));
}

#[test]
fn invalid_toml_and_bad_types_are_invalid_with_path() {
    let temp = TempDir::new().expect("tempdir");
    let root = root_of(&temp);
    let file = temp.path().join("fs-ez.toml");

    std::fs::write(&file, "port = [").expect("écriture");
    let error = load_file_config(&root).expect_err("toml invalide");
    assert!(
        matches!(error, fs_ez_app::AppError::Invalid(ref message) if message.contains("fs-ez.toml"))
    );

    std::fs::write(&file, "port = \"high\"\n").expect("écriture");
    let error = load_file_config(&root).expect_err("type invalide");
    assert!(
        matches!(error, fs_ez_app::AppError::Invalid(ref message) if message.contains("fs-ez.toml"))
    );
}

#[test]
fn parent_segments_are_rejected() {
    let temp = TempDir::new().expect("tempdir");
    std::fs::write(temp.path().join("fs-ez.toml"), "db = \"a/../escape.db\"\n").expect("écriture");
    let error = load_file_config(&root_of(&temp)).expect_err("échappement");
    assert!(matches!(error, fs_ez_app::AppError::Invalid(_)));
}
