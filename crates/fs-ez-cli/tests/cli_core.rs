//! Tests d'intégration des commandes de base de la CLI.

mod common;

use common::{Harness, items};
use fs_ez_cli::error::CliError;
use serde_json::Value;

#[test]
fn init_then_tag_and_query() {
    let harness = Harness::new();
    let init = harness.json(&["init"]);
    assert_eq!(init["schema"], Value::from(1));
    assert!(init["root"].as_str().is_some());
    assert!(init["db"].as_str().is_some());
    assert!(init["files"].as_str().is_some());

    let path = harness.write("facture.pdf", b"contenu");
    let tagged = harness.json(&["tag", "--tags=facture,2026", path.to_str().expect("utf8")]);
    assert_eq!(tagged["files"].as_array().expect("files").len(), 1);
    assert_eq!(items(&harness.json(&["files", "facture"])).len(), 1);
    assert_eq!(items(&harness.json(&["files", "2026"])).len(), 1);
    assert_eq!(items(&harness.json(&["files", "inconnu"])).len(), 0);
}

#[test]
fn untag_removes_tag() {
    let harness = Harness::new();
    let path = harness.write("a.txt", b"a");
    let arg = path.to_str().expect("utf8");
    harness.json(&["tag", "--tags=facture", arg]);
    harness.json(&["untag", "--tags=facture", arg]);
    assert_eq!(items(&harness.json(&["files", "facture"])).len(), 0);
}

#[test]
fn rename_merge_delete() {
    let harness = Harness::new();
    let path = harness.write("c.txt", b"c");
    let arg = path.to_str().expect("utf8");
    harness.json(&["tag", "--tags=ancien,doublon", arg]);
    let renamed = harness.json(&["rename", "ancien", "nouveau"]);
    assert_eq!(renamed["tag"]["name"].as_str(), Some("nouveau"));
    assert_eq!(items(&harness.json(&["files", "nouveau"])).len(), 1);
    assert_eq!(items(&harness.json(&["files", "ancien"])).len(), 0);

    let merged = harness.json(&["merge", "--into=nouveau", "doublon"]);
    assert_eq!(merged["ok"], Value::Bool(true));
    assert_eq!(items(&harness.json(&["files", "doublon"])).len(), 0);

    let deleted = harness.json(&["delete", "nouveau"]);
    assert_eq!(deleted["deleted"], Value::from(1));
    assert!(
        harness
            .error(&["delete", "nouveau"])
            .to_string()
            .contains("not found")
    );
}

#[test]
fn files_untagged_recent_and_pagination() {
    let harness = Harness::new();
    let a = harness.write("a.txt", b"a");
    let b = harness.write("b.txt", b"b");
    let c = harness.write("c.txt", b"c");
    for path in [&a, &b, &c] {
        harness.json(&["tag", "--tags=temp", path.to_str().expect("utf8")]);
        harness.json(&["untag", "--tags=temp", path.to_str().expect("utf8")]);
    }
    assert_eq!(items(&harness.json(&["files", "--untagged"])).len(), 3);

    harness.json(&["tag", "--tags=2026", a.to_str().expect("utf8")]);
    assert_eq!(items(&harness.json(&["files", "--untagged"])).len(), 2);
    assert_eq!(items(&harness.json(&["files", "--recent"])).len(), 1);

    harness.json(&["tag", "--tags=pagination", b.to_str().expect("utf8")]);
    harness.json(&["tag", "--tags=pagination", c.to_str().expect("utf8")]);

    let first = harness.json(&["files", "pagination", "--limit=1"]);
    assert_eq!(items(&first).len(), 1);
    let offset = first["next_offset"].as_u64().expect("offset").to_string();
    let second = harness.json(&["files", "pagination", "--limit=1", "--offset", &offset]);
    assert_eq!(items(&second).len(), 1);
}

#[test]
fn status_counts_and_repair_moves() {
    let harness = Harness::new();
    let path = harness.write("mobile.txt", b"mobile");
    let arg = path.to_str().expect("utf8");
    harness.json(&["tag", "--tags=facture", arg]);

    let status = harness.json(&["status"]);
    assert_eq!(status["files"], Value::from(1));
    assert_eq!(status["tags"], Value::from(1));
    assert_eq!(status["untagged"], Value::from(0));

    let moved = harness.temp.path().join("deplace.txt");
    std::fs::rename(&path, &moved).expect("déplacement");
    let repair = harness.json(&["repair"]);
    assert_eq!(repair["moved"], Value::from(1));
    assert_eq!(repair["missing"], Value::from(0));
}

#[test]
fn config_set_and_list() {
    let harness = Harness::new();
    harness.json(&["config", "set", "ui.recent_days", "3"]);
    let listed = harness.json(&["config", "list"]);
    let settings = listed["settings"].as_array().expect("settings");
    let entry = settings
        .iter()
        .find(|entry| entry["key"].as_str() == Some("ui.recent_days"))
        .expect("clé présente");
    assert_eq!(entry["value"].as_str(), Some("3"));
    let got = harness.json(&["config", "get", "ui.recent_days"]);
    assert_eq!(got["value"].as_str(), Some("3"));
    let missing = harness.json(&["config", "get", "ui.inexistant"]);
    assert!(missing["value"].is_null());
}

#[test]
fn json_outputs_are_versioned() {
    let harness = Harness::new();
    let path = harness.write("json.txt", b"json");
    let arg = path.to_str().expect("utf8");
    let cases: Vec<Vec<&str>> = vec![
        vec!["init"],
        vec!["tag", "--tags=facture,a,b", arg],
        vec!["untag", "--tags=facture", arg],
        vec!["files"],
        vec!["tags"],
        vec!["rename", "a", "c"],
        vec!["merge", "--into=c", "b"],
        vec!["delete", "c"],
        vec!["repair"],
        vec!["status"],
        vec!["config", "list"],
        vec!["config", "get", "ui.recent_days"],
        vec!["config", "set", "ui.recent_days", "3"],
        vec!["help"],
    ];
    for case in cases {
        let value = harness.rendered_json(&case);
        assert_eq!(value["schema"], Value::from(1), "commande {case:?}");
    }
}

#[test]
fn usage_and_unknown_tag_fail() {
    let harness = Harness::new();
    assert!(matches!(
        harness.error(&["files", "--limit=0"]),
        CliError::Usage(_)
    ));
    assert!(matches!(
        harness.error(&["tag", "sans-tags"]),
        CliError::Usage(_)
    ));
    assert!(matches!(
        harness.error(&["rename", "inconnu", "autre"]),
        CliError::App(_)
    ));
    assert!(matches!(
        harness.error(&["files", "--untagged", "--recent"]),
        CliError::Usage(_)
    ));
}
