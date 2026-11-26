use super::*;
use tempfile::TempDir;

fn map(extra: &[(&str, &str)]) -> HashMap<String, String> {
    let mut values = HashMap::new();
    for (key, value) in extra {
        values.insert((*key).to_string(), (*value).to_string());
    }
    values
}

#[test]
fn defaults_without_env_or_file() {
    let temp = TempDir::new().expect("tempdir");
    let config = WebConfig::from_map(&map(&[]), temp.path()).expect("config");
    assert_eq!(config.host, "0.0.0.0");
    assert_eq!(config.port, 8678);
    assert_eq!(config.max_upload, DEFAULT_MAX_UPLOAD);
    assert_eq!(config.root, temp.path().canonicalize().expect("canonique"));
    assert_eq!(config.db_path, config.root.join("config/fs-ez.db"));
    assert!(config.files.is_none());
}

#[test]
fn file_db_path_is_resolved_under_root() {
    let temp = TempDir::new().expect("tempdir");
    std::fs::write(temp.path().join("fs-ez.toml"), "db = \"state/fs-ez.db\"\n").expect("écriture");
    let config = WebConfig::from_map(&map(&[]), temp.path()).expect("config");
    let root = temp.path().canonicalize().expect("canonique");
    assert_eq!(config.db_path, root.join("state/fs-ez.db"));
}

#[test]
fn file_layer_supplies_host_port_and_env_wins() {
    let temp = TempDir::new().expect("tempdir");
    std::fs::write(
        temp.path().join("fs-ez.toml"),
        "host = \"127.0.0.1\"\nport = 9000\nmax_upload = 1024\n",
    )
    .expect("écriture");
    let config = WebConfig::from_map(&map(&[]), temp.path()).expect("config");
    assert_eq!(config.host, "127.0.0.1");
    assert_eq!(config.port, 9000);
    assert_eq!(config.max_upload, 1024);

    let config = WebConfig::from_map(&map(&[("PORT", "9100"), ("HOST", "0.0.0.0")]), temp.path())
        .expect("config");
    assert_eq!(config.host, "0.0.0.0");
    assert_eq!(config.port, 9100);
}

#[test]
fn invalid_toml_is_config_error() {
    let temp = TempDir::new().expect("tempdir");
    std::fs::write(temp.path().join("fs-ez.toml"), "port = \"abc\"\n").expect("écriture");
    let error = WebConfig::from_map(&map(&[]), temp.path()).expect_err("erreur");
    assert!(matches!(error, WebError::Config(_)));
}

#[test]
fn strict_variables_reject_bad_values() {
    let temp = TempDir::new().expect("tempdir");
    for (key, value) in [
        ("PORT", "abc"),
        ("PORT", "70000"),
        ("MAX_UPLOAD", "abc"),
        ("MAX_UPLOAD", "0"),
    ] {
        let error = WebConfig::from_map(&map(&[(key, value)]), temp.path()).expect_err(key);
        match error {
            WebError::Config(message) => {
                assert!(message.contains(key), "{message}");
                assert!(message.contains(value), "{message}");
            }
            other => panic!("attendu Config, reçu {other:?}"),
        }
    }
}

#[test]
fn file_files_path_is_resolved_under_root() {
    let temp = TempDir::new().expect("tempdir");
    std::fs::write(temp.path().join("fs-ez.toml"), "files = \"stored\"\n").expect("écriture");
    let config = WebConfig::from_map(&map(&[]), temp.path()).expect("config");
    let root = temp.path().canonicalize().expect("canonique");
    assert_eq!(config.files, Some(root.join("stored")));
}

#[test]
fn max_upload_defaults_to_sixteen_gib() {
    let temp = TempDir::new().expect("tempdir");
    let config = WebConfig::from_map(&map(&[]), temp.path()).expect("config");
    assert_eq!(config.max_upload, 16 * 1024 * 1024 * 1024);
}

#[test]
fn env_listen_wins_over_file_and_cli_listen_wins_over_env() {
    let temp = TempDir::new().expect("tempdir");
    std::fs::write(
        temp.path().join("fs-ez.toml"),
        "host = \"127.0.0.1\"\nport = 9000\n",
    )
    .expect("écriture");

    let config = WebConfig::from_map(&map(&[("HOST", "0.0.0.0"), ("PORT", "9100")]), temp.path())
        .expect("config");
    assert_eq!(config.host, "0.0.0.0");
    assert_eq!(config.port, 9100);

    let config = config.with_listen(Some("192.0.2.10".to_string()), Some(9200));
    assert_eq!(config.host, "192.0.2.10");
    assert_eq!(config.port, 9200);
}

#[test]
fn explicit_root_and_db_win_over_file() {
    let cli_root = TempDir::new().expect("tempdir");
    std::fs::write(
        cli_root.path().join("fs-ez.toml"),
        "host = \"0.0.0.0\"\nport = 18991\nmax_upload = 1024\n",
    )
    .expect("écriture");

    let values = map(&[("HOST", "127.0.0.1"), ("PORT", "18992")]);
    let cli_db = cli_root.path().join("cli.db");
    let config = WebConfig::from_map_with(&values, cli_root.path(), &cli_db).expect("config");

    assert_eq!(
        config.root,
        cli_root.path().canonicalize().expect("canonique")
    );
    assert_eq!(config.db_path, cli_db);
    assert_eq!(config.host, "127.0.0.1");
    assert_eq!(config.port, 18992);
    assert_eq!(config.max_upload, 1024);
}
