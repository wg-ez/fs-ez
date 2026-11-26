//! Tests d'intégration des commandes étendues de la CLI.

mod common;

use common::{Harness, items};
use fs_ez_cli::error::CliError;
use serde_json::Value;

fn seeded_tags(harness: &Harness) {
    let path = harness.write("seed.txt", b"seed");
    harness.json(&["tag", "--tags=rust,sql", path.to_str().expect("utf8")]);
}

fn user_reset_with_stdin(harness: &Harness, username: Option<&str>, password: &str) -> Value {
    let mut command = assert_cmd::Command::new(common::fs_ez_bin());
    command
        .env("NO_COLOR", "1")
        .current_dir(harness.temp.path())
        .arg("--json")
        .arg("user")
        .arg("reset");
    if let Some(username) = username {
        command.arg(format!("--username={username}"));
    }
    command.write_stdin(format!("{password}\n"));
    let output = command.output().expect("exécution fs-ez");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("JSON valide")
}

#[test]
fn group_lifecycle() {
    let harness = Harness::new();
    harness.init();
    seeded_tags(&harness);

    let created = harness.json(&["group", "add", "projet", "--color=blue"]);
    assert_eq!(created["group"]["name"].as_str(), Some("projet"));
    assert_eq!(created["group"]["color"].as_str(), Some("blue"));
    assert_eq!(created["group"]["tags"].as_array().expect("tags").len(), 0);

    harness.json(&["group", "add", "autre"]);
    harness.json(&["group", "assign", "projet", "rust"]);
    harness.json(&["group", "assign", "projet", "sql"]);

    let listed = harness.json(&["group", "list"]);
    let groups = listed["groups"].as_array().expect("groups");
    let projet = groups
        .iter()
        .find(|group| group["name"].as_str() == Some("projet"))
        .expect("groupe projet");
    assert_eq!(projet["tags"].as_array().expect("tags").len(), 2);

    harness.json(&["group", "move", "projet", "autre", "rust"]);
    let moved = harness.json(&["group", "list"]);
    let groups = moved["groups"].as_array().expect("groups");
    let projet = groups
        .iter()
        .find(|group| group["name"].as_str() == Some("projet"))
        .expect("groupe projet");
    let autre = groups
        .iter()
        .find(|group| group["name"].as_str() == Some("autre"))
        .expect("groupe autre");
    assert_eq!(projet["tags"].as_array().expect("tags").len(), 1);
    assert_eq!(autre["tags"].as_array().expect("tags").len(), 1);

    let removed = harness.json(&["group", "rm", "autre"]);
    assert_eq!(removed["ok"], Value::Bool(true));
    harness.json(&["group", "rm", "Divers"]);
    let listed = harness.json(&["group", "list"]);
    assert_eq!(listed["groups"].as_array().expect("groups").len(), 1);
}

#[test]
fn color_set_accepts_tokens_and_rejects_unknown() {
    let harness = Harness::new();
    harness.init();
    seeded_tags(&harness);

    let tagged = harness.json(&["color", "set", "rust", "amber"]);
    assert_eq!(tagged["tag"]["color"].as_str(), Some("amber"));

    let unknown = harness.error(&["color", "set", "rust", "mauve"]);
    assert!(matches!(unknown, CliError::Usage(message) if message.contains("neutral")));
}

#[test]
fn view_add_filter_query_list_run_rm() {
    let harness = Harness::new();
    harness.init();
    let rust = harness.write("rust.txt", b"rust");
    let other = harness.write("other.txt", b"other");
    harness.json(&["tag", "--tags=rust", rust.to_str().expect("utf8")]);
    harness.json(&["tag", "--tags=sql", other.to_str().expect("utf8")]);

    let by_filter = harness.json(&[
        "view",
        "add",
        "RustFilter",
        "--filter",
        r#"{"op":"tag","tag":"rust"}"#,
    ]);
    assert_eq!(by_filter["view"]["name"].as_str(), Some("RustFilter"));
    assert_eq!(by_filter["view"]["query"].as_str(), Some("rust"));

    let by_query = harness.json(&["view", "add", "RustQuery", "--query", "rust"]);
    assert_eq!(by_query["view"]["name"].as_str(), Some("RustQuery"));

    let listed = harness.json(&["view", "list"]);
    assert_eq!(listed["views"].as_array().expect("views").len(), 2);

    let run = harness.json(&["view", "run", "RustFilter"]);
    assert_eq!(items(&run).len(), 1);

    let removed = harness.json(&["view", "rm", "RustQuery"]);
    assert_eq!(removed["ok"], Value::Bool(true));
    let listed = harness.json(&["view", "list"]);
    assert_eq!(listed["views"].as_array().expect("views").len(), 1);

    let invalid = harness.error(&["view", "add", "Bad", "--filter", "{nope"]);
    assert!(matches!(invalid, CliError::Json(_)));
}

#[test]
fn inbox_scan_imports_and_consumes_drop() {
    let harness = Harness::new();
    harness.init();
    let first = harness.write("inbox/one.txt", b"one");
    let second = harness.write("inbox/two.txt", b"two");

    let scanned = harness.json(&["inbox", "scan"]);
    assert_eq!(scanned["imported"], Value::from(2));
    assert_eq!(scanned["duplicates"], Value::from(0));
    assert!(!first.exists(), "dépôt consommé");
    assert!(!second.exists(), "dépôt consommé");
    assert_eq!(items(&harness.json(&["files"])).len(), 2);
}

#[test]
fn token_add_list_revoke() {
    let harness = Harness::new();
    harness.init();
    let created = harness.json(&["token", "add", "ci", "--expires=2030-01-01T00:00:00Z"]);
    let token = created["token"].as_str().expect("token");
    assert_eq!(token.chars().count(), 43);
    assert_eq!(created["name"].as_str(), Some("ci"));
    assert_eq!(created["expires_at"].as_str(), Some("2030-01-01T00:00:00Z"));
    let id = created["id"].as_i64().expect("id");

    let listed = harness.json(&["token", "list"]);
    let tokens = listed["tokens"].as_array().expect("tokens");
    assert_eq!(tokens.len(), 1);
    assert!(tokens[0].get("token").is_none());
    assert_eq!(tokens[0]["name"].as_str(), Some("ci"));

    let revoked = harness.json(&["token", "revoke", &id.to_string()]);
    assert_eq!(revoked["ok"], Value::Bool(true));
    let listed = harness.json(&["token", "list"]);
    assert_eq!(listed["tokens"].as_array().expect("tokens").len(), 0);
}

#[test]
fn user_reset_then_app_login() {
    let harness = Harness::new();
    harness.init();
    let password = "correct horse battery";
    let reset = user_reset_with_stdin(&harness, Some("root"), password);
    assert_eq!(reset["ok"], Value::Bool(true));
    assert_eq!(reset["username"].as_str(), Some("root"));
    assert!(reset.get("password").is_none());
    assert!(reset.get("password_hash").is_none());
    assert!(harness.ctx.app.login("root", password).is_ok());
}

#[test]
fn every_extended_command_emits_versioned_json() {
    let harness = Harness::new();
    harness.init();
    seeded_tags(&harness);
    harness.write("files/inbox.txt", b"inbox");

    let cases: Vec<Vec<String>> = vec![
        vec!["group".into(), "list".into()],
        vec!["group".into(), "add".into(), "g1".into()],
        vec!["group".into(), "assign".into(), "g1".into(), "rust".into()],
        vec![
            "group".into(),
            "move".into(),
            "g1".into(),
            "g1".into(),
            "rust".into(),
        ],
        vec!["color".into(), "set".into(), "rust".into(), "green".into()],
        vec!["group".into(), "rm".into(), "g1".into()],
        vec!["view".into(), "list".into()],
        vec![
            "view".into(),
            "add".into(),
            "V".into(),
            "--query".into(),
            "rust".into(),
        ],
        vec!["view".into(), "run".into(), "V".into()],
        vec!["view".into(), "rm".into(), "V".into()],
        vec!["inbox".into(), "scan".into()],
        vec!["token".into(), "add".into(), "ci".into()],
        vec!["token".into(), "list".into()],
    ];
    for case in cases {
        let args: Vec<&str> = case.iter().map(String::as_str).collect();
        assert_eq!(
            harness.rendered_json(&args)["schema"],
            Value::from(1),
            "commande {args:?}"
        );
    }

    let tokens = harness.json(&["token", "list"]);
    let token_id = tokens["tokens"][0]["id"].as_i64().expect("id").to_string();
    let revoke = vec!["token", "revoke", &token_id];
    assert_eq!(harness.rendered_json(&revoke)["schema"], Value::from(1));

    let reset = user_reset_with_stdin(&harness, None, "correct horse battery");
    assert_eq!(reset["schema"], Value::from(1));
}
