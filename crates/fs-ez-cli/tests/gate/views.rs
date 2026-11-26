use super::*;

#[test]
fn saved_views_add_run_list_rm() {
    let fixture = CliFixture::new();
    fixture.ok(&["init"]);
    let path = fixture.write("view.txt", b"view");
    fixture.ok(&["tag", "--tags=rust", path.to_str().expect("utf8")]);

    let added = fixture.json(&["view", "add", "Rust", "--query", "rust"]);
    assert_eq!(added["view"]["name"], json!("Rust"));

    let run = fixture.json(&["view", "run", "Rust"]);
    assert_eq!(items(&run).len(), 1);

    let listed = fixture.json(&["view", "list"]);
    assert_eq!(listed["views"].as_array().expect("views").len(), 1);

    let removed = fixture.json(&["view", "rm", "Rust"]);
    assert_eq!(removed["ok"], json!(true));
    assert_eq!(
        fixture.json(&["view", "list"])["views"]
            .as_array()
            .expect("views")
            .len(),
        0
    );
}

#[test]
fn groups_and_colors_round_trip() {
    let fixture = CliFixture::new();
    fixture.ok(&["init"]);
    let path = fixture.write("group.txt", b"group");
    fixture.ok(&["tag", "--tags=rust", path.to_str().expect("utf8")]);

    fixture.ok(&["group", "add", "langages", "--color=blue"]);
    fixture.ok(&["group", "assign", "langages", "rust"]);
    let listed = fixture.json(&["group", "list"]);
    let group = &listed["groups"].as_array().expect("groups")[0];
    assert_eq!(group["name"], json!("langages"));
    assert_eq!(group["tags"].as_array().expect("tags").len(), 1);

    fixture.ok(&["color", "set", "rust", "amber"]);
    let tags = fixture.json(&["tags"]);
    assert_eq!(tags["tags"][0]["color"], json!("amber"));
}
