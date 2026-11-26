use super::*;

#[test]
fn tagging_cycle_covers_core_commands() {
    let fixture = CliFixture::new();
    fixture.ok(&["init"]);
    let first = fixture.write("first.txt", b"first");
    let second = fixture.write("second.txt", b"second");
    let first_arg = first.to_str().expect("utf8");
    let second_arg = second.to_str().expect("utf8");

    let tagged = fixture.json(&["tag", "--tags=alpha,beta", first_arg]);
    assert_eq!(tagged["files"].as_array().expect("files").len(), 1);
    assert_eq!(items(&fixture.json(&["files", "alpha"])).len(), 1);

    fixture.ok(&["untag", "--tags=alpha", first_arg]);
    assert_eq!(items(&fixture.json(&["files", "alpha"])).len(), 0);

    fixture.ok(&["rename", "beta", "gamma"]);
    assert_eq!(items(&fixture.json(&["files", "gamma"])).len(), 1);
    assert_eq!(items(&fixture.json(&["files", "beta"])).len(), 0);
    assert_eq!(
        fixture.json(&["files", "gamma"])["items"][0]["tags"]
            .as_array()
            .expect("tags")
            .len(),
        1
    );

    fixture.ok(&["tag", "--tags=epsilon,delta", second_arg]);
    assert_eq!(items(&fixture.json(&["files", "delta"])).len(), 1);

    let merged = fixture.json(&["merge", "--into=gamma", "epsilon"]);
    assert!(merged["files"].as_u64().expect("compte") >= 1);
    assert_eq!(items(&fixture.json(&["files", "epsilon"])).len(), 0);
    assert_eq!(items(&fixture.json(&["files", "gamma"])).len(), 2);

    let deleted = fixture.json(&["delete", "gamma"]);
    assert_eq!(deleted["deleted"], json!(1));
    assert_eq!(items(&fixture.json(&["files", "gamma"])).len(), 0);
}

#[test]
fn repair_relocates_a_moved_file() {
    let fixture = CliFixture::new();
    fixture.ok(&["init"]);
    let original = fixture.write("mobile.txt", b"mobile content");
    fixture.ok(&["tag", "--tags=facture", original.to_str().expect("utf8")]);

    let moved = fixture.root.join("moved.txt");
    fs::rename(&original, &moved).expect("déplacement");
    let report = fixture.json(&["repair"]);
    assert_eq!(report["moved"], json!(1));
    assert_eq!(report["missing"], json!(0));

    let files = fixture.json(&["files", "facture"]);
    assert_eq!(
        items(&files)[0]["file"]["path_display"],
        json!(moved.to_string_lossy())
    );
}
