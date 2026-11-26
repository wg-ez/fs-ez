use std::fs;
use std::os::unix::fs::PermissionsExt;

use super::*;

#[test]
fn init_is_idempotent_and_private() {
    let fixture = CliFixture::new();
    fixture.ok(&["init"]);
    fixture.ok(&["init"]);

    let state = fixture.root.join("config");
    assert_eq!(
        fs::metadata(&state).expect("état").permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(&fixture.db).expect("db").permissions().mode() & 0o777,
        0o600
    );
    assert!(fixture.root.join("files").is_dir());
}

#[test]
fn explicit_root_and_db_override_environment() {
    let fixture = CliFixture::new();
    fixture.ok(&["init"]);
    let other_root = fixture.root.parent().expect("parent").join("other-root");
    fs::create_dir_all(&other_root).expect("autre root");
    let other_root = fs::canonicalize(&other_root).expect("canon");
    let other_db = other_root.join("config").join("custom.db");

    let mut command = fixture.command();
    let run = fixture.run_with(
        &mut command,
        &[
            "init",
            "--root",
            other_root.to_str().expect("utf8"),
            "--db",
            other_db.to_str().expect("utf8"),
            "--json",
        ],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    let value: Value = serde_json::from_str(&run.stdout).expect("JSON");
    assert_eq!(value["root"], json!(other_root.to_string_lossy()));
    assert_eq!(value["db"], json!(other_db.to_string_lossy()));
    assert!(other_db.exists());

    let mut command = fixture.command();
    let status = fixture.run_with(
        &mut command,
        &[
            "status",
            "--root",
            other_root.to_str().expect("utf8"),
            "--db",
            other_db.to_str().expect("utf8"),
            "--json",
        ],
    );
    assert_eq!(status.code, 0, "{}", status.stderr);
    let value: Value = serde_json::from_str(&status.stdout).expect("JSON");
    assert_eq!(value["db_path"], json!(other_db.to_string_lossy()));
}
