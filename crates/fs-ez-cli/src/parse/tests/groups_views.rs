use super::*;

#[test]
fn group_variants() {
    assert_eq!(
        parsed(&["group", "add", "projet", "--color=blue"]).command,
        Command::GroupAdd {
            name: "projet".into(),
            color: Some("blue".into())
        }
    );
    assert_eq!(
        parsed(&["group", "rm", "projet"]).command,
        Command::GroupRm {
            name: "projet".into()
        }
    );
    assert_eq!(
        parsed(&["group", "assign", "projet", "rust"]).command,
        Command::GroupAssign {
            group: "projet".into(),
            tag: "rust".into()
        }
    );
    assert_eq!(
        parsed(&["group", "move", "a", "b", "rust"]).command,
        Command::GroupMove {
            from: "a".into(),
            to: "b".into(),
            tag: "rust".into()
        }
    );
    assert_eq!(
        parsed(&["color", "set", "rust", "amber"]).command,
        Command::ColorSet {
            tag: "rust".into(),
            token: "amber".into()
        }
    );
}

#[test]
fn view_add_with_raw_json_filter() {
    let filter = r#"{"and":[{"tag":"rust"}]}"#;
    let result = parsed(&[
        "view",
        "add",
        "Rust",
        "--filter",
        filter,
        "--sort=size:asc",
        "--icon=bookmark",
        "--color=blue",
    ]);
    match result.command {
        Command::ViewAdd {
            name,
            filter: parsed_filter,
            sort,
            icon,
            color,
            ..
        } => {
            assert_eq!(name, "Rust");
            assert_eq!(parsed_filter.as_deref(), Some(filter));
            assert_eq!(
                sort,
                Sort {
                    field: SortField::Size,
                    dir: SortDir::Asc
                }
            );
            assert_eq!(icon.as_deref(), Some("bookmark"));
            assert_eq!(color.as_deref(), Some("blue"));
        }
        other => panic!("unexpected command {other:?}"),
    }
}

#[test]
fn view_run_and_rm() {
    assert_eq!(
        parsed(&["view", "rm", "Rust"]).command,
        Command::ViewRm {
            name: "Rust".into()
        }
    );
    assert_eq!(
        parsed(&[
            "view",
            "run",
            "Rust",
            "--limit=10",
            "--offset=3",
            "--print0"
        ])
        .command,
        Command::ViewRun {
            name: "Rust".into(),
            limit: 10,
            offset: 3,
            print0: true
        }
    );
}
