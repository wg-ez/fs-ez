use super::*;

#[test]
fn snapshot_files_json() {
    let fixture = CliFixture::new();
    fixture.ok(&["init"]);
    for (name, tags, bytes) in [
        ("a.txt", "alpha,shared", b"aaa".as_slice()),
        ("b.txt", "beta,shared", b"bbbb".as_slice()),
        ("c.txt", "gamma,shared", b"ccccc".as_slice()),
    ] {
        let path = fixture.write(name, bytes);
        fixture.ok(&[
            "tag",
            &format!("--tags={tags}"),
            path.to_str().expect("utf8"),
        ]);
    }
    for tag in ["alpha", "beta", "gamma", "shared"] {
        fixture.ok(&["color", "set", tag, "neutral"]);
    }
    // Nom et dossier conservés : module déplacé, préfixe de module neutralisé.
    insta::with_settings!(
        { prepend_module_to_snapshot => false, snapshot_path => "../snapshots" },
        {
            insta::assert_json_snapshot!(
                "gate__files_json",
                normalized(&fixture, &["files", "--sort=name"])
            );
        }
    );
}

#[test]
fn snapshot_tags_json() {
    let fixture = CliFixture::new();
    fixture.ok(&["init"]);
    let path = fixture.write("tagged.txt", b"tagged");
    fixture.ok(&["tag", "--tags=alpha,beta", path.to_str().expect("utf8")]);
    for tag in ["alpha", "beta"] {
        fixture.ok(&["color", "set", tag, "neutral"]);
    }
    insta::with_settings!(
        { prepend_module_to_snapshot => false, snapshot_path => "../snapshots" },
        {
            insta::assert_json_snapshot!("gate__tags_json", normalized(&fixture, &["tags"]));
        }
    );
}

#[test]
fn snapshot_status_json() {
    let fixture = CliFixture::new();
    fixture.ok(&["init"]);
    let path = fixture.write("status.txt", b"status");
    fixture.ok(&["tag", "--tags=alpha", path.to_str().expect("utf8")]);
    insta::with_settings!(
        { prepend_module_to_snapshot => false, snapshot_path => "../snapshots" },
        {
            insta::assert_json_snapshot!("gate__status_json", normalized(&fixture, &["status"]));
        }
    );
}

#[test]
fn snapshot_view_list_json() {
    let fixture = CliFixture::new();
    fixture.ok(&["init"]);
    fixture.ok(&["view", "add", "Rust", "--query", "rust", "--color=blue"]);
    insta::with_settings!(
        { prepend_module_to_snapshot => false, snapshot_path => "../snapshots" },
        {
            insta::assert_json_snapshot!(
                "gate__view_list_json",
                normalized(&fixture, &["view", "list"])
            );
        }
    );
}
