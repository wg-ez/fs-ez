//! Codes de sortie du binaire `fs-ez`.

use std::process::{Command, Output};

use tempfile::TempDir;

fn shell(root: &TempDir, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fs-ez"))
        .arg("--root")
        .arg(root.path())
        .args(args)
        .output()
        .expect("exécution")
}

#[test]
fn version_and_init_succeed() {
    let root = TempDir::new().expect("tempdir");
    let version = shell(&root, &["version"]);
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("fs-ez"));

    let init = shell(&root, &["--json", "init"]);
    assert!(init.status.success());
    let value: serde_json::Value = serde_json::from_slice(&init.stdout).expect("JSON valide");
    assert_eq!(value["schema"], serde_json::Value::from(1));
}

#[test]
fn invalid_usage_and_unknown_tag_exit_one() {
    let root = TempDir::new().expect("tempdir");
    let usage = shell(&root, &["tag", "sans-tags"]);
    assert_eq!(usage.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&usage.stderr).contains("usage"));

    let unknown = shell(&root, &["rename", "inconnu", "autre"]);
    assert_eq!(unknown.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("not found"));
}

#[test]
fn no_argument_and_unknown_command_show_usage() {
    let root = TempDir::new().expect("tempdir");
    let empty = Command::new(env!("CARGO_BIN_EXE_fs-ez"))
        .output()
        .expect("exécution");
    assert_eq!(empty.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&empty.stderr).contains("usage:"));

    let unknown = shell(&root, &["commande-inconnue"]);
    assert_eq!(unknown.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&unknown.stderr);
    assert!(stderr.contains("unknown command 'commande-inconnue'"));
    assert!(stderr.contains("usage:"));
}

#[test]
fn stderr_neutralizes_terminal_escape_sequences() {
    let root = TempDir::new().expect("tempdir");
    let unknown = shell(&root, &["commande-\u{1b}[2J"]);
    assert_eq!(unknown.status.code(), Some(1));
    assert!(
        !unknown.stderr.contains(&0x1b),
        "ESC brut dans stderr : {:?}",
        String::from_utf8_lossy(&unknown.stderr)
    );
    assert!(
        String::from_utf8_lossy(&unknown.stderr).contains("commande-?[2J"),
        "{:?}",
        String::from_utf8_lossy(&unknown.stderr)
    );

    let sort = shell(&root, &["files", "--sort=id\u{1b}[2J"]);
    assert_eq!(sort.status.code(), Some(1));
    assert!(!sort.stderr.contains(&0x1b));
    assert!(String::from_utf8_lossy(&sort.stderr).contains("id?[2J"));

    let normal = shell(&root, &["commande-inconnue"]);
    assert!(
        String::from_utf8_lossy(&normal.stderr).contains("unknown command 'commande-inconnue'")
    );
}
