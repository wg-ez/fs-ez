//! Commandes de tags en lot : ordre, doublons et sorties identiques.

mod common;

use common::Harness;
use serde_json::Value;

#[test]
fn tag_and_untag_keep_duplicate_paths_in_order() {
    let harness = Harness::new();
    let first = harness.write("un.txt", b"un");
    let second = harness.write("deux.txt", b"deux");
    let first = first.to_str().expect("utf8");
    let second = second.to_str().expect("utf8");

    let tagged = harness.output(&["tag", "--tags=lot", first, second, first]);
    let files = tagged.json["files"].as_array().expect("files");
    assert_eq!(files.len(), 3);
    assert_eq!(files[0]["file"]["id"], files[2]["file"]["id"]);
    assert_ne!(files[0]["file"]["id"], files[1]["file"]["id"]);
    assert_eq!(files[0]["tags"].as_array().expect("tags").len(), 1);
    assert_eq!(tagged.human.lines().count(), 4);
    assert_eq!(
        harness.json(&["tags"])["tags"]
            .as_array()
            .expect("tags")
            .len(),
        1
    );

    let untagged = harness.output(&["untag", "--tags=lot", first, second, first]);
    let files = untagged.json["files"].as_array().expect("files");
    assert_eq!(files.len(), 3);
    for file in files {
        assert!(file["tags"].as_array().expect("tags").is_empty());
    }
    assert_eq!(untagged.human.lines().count(), 4);
}

#[test]
fn tag_batch_unknown_path_fails_like_the_unit_path() {
    let harness = Harness::new();
    let missing = harness.temp.path().join("absent.txt");
    let missing = missing.to_str().expect("utf8");

    let error = harness.error(&["tag", "--tags=lot", missing]);
    assert!(error.to_string().contains("not found"));
    assert_eq!(
        harness.json(&["tags"])["tags"]
            .as_array()
            .expect("tags")
            .len(),
        0
    );

    let forced = harness.json(&["tag", "--force", "--tags=lot", missing]);
    let files = forced["files"].as_array().expect("files");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["file"]["size"], Value::from(0));
    assert_eq!(files[0]["tags"].as_array().expect("tags").len(), 1);
}
