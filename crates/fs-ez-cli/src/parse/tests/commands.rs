use super::*;

#[test]
fn simple_commands() {
    assert_eq!(parsed(&["init"]).command, Command::Init);
    assert_eq!(parsed(&["tags"]).command, Command::Tags);
    assert_eq!(parsed(&["repair"]).command, Command::Repair);
    assert_eq!(parsed(&["status"]).command, Command::Status);
    assert_eq!(parsed(&["group", "list"]).command, Command::GroupList);
    assert_eq!(parsed(&["view", "list"]).command, Command::ViewList);
    assert_eq!(parsed(&["token", "list"]).command, Command::TokenList);
    assert_eq!(parsed(&["inbox", "scan"]).command, Command::InboxScan);
    assert_eq!(parsed(&["version"]).command, Command::Version);
    assert_eq!(parsed(&["--version"]).command, Command::Version);
    assert_eq!(parsed(&["--help"]).command, Command::Help(None));
    assert_eq!(
        parsed(&["tag", "--help"]).command,
        Command::Help(Some("tag".into()))
    );
    assert_eq!(
        parsed(&["help", "tag"]).command,
        Command::Help(Some("tag".into()))
    );
}

#[test]
fn rename_merge_delete() {
    assert_eq!(
        parsed(&["rename", "a", "b"]).command,
        Command::Rename {
            old: "a".into(),
            new: "b".into()
        }
    );
    assert_eq!(
        parsed(&["merge", "--into=target", "s1", "s2"]).command,
        Command::Merge {
            sources: vec!["s1".into(), "s2".into()],
            target: "target".into()
        }
    );
    assert_eq!(
        parsed(&["delete", "a", "b"]).command,
        Command::Delete {
            names: vec!["a".into(), "b".into()]
        }
    );
}

#[test]
fn config_variants() {
    assert_eq!(parsed(&["config", "list"]).command, Command::ConfigList);
    assert_eq!(
        parsed(&["config", "get", "theme"]).command,
        Command::ConfigGet {
            key: "theme".into()
        }
    );
    assert_eq!(
        parsed(&["config", "set", "theme", "dark"]).command,
        Command::ConfigSet {
            key: "theme".into(),
            value: "dark".into()
        }
    );
}

#[test]
fn serve_host_and_port() {
    assert_eq!(
        parsed(&["serve", "--host", "0.0.0.0", "--port=8080"]).command,
        Command::Serve {
            host: Some("0.0.0.0".into()),
            port: Some(8080)
        }
    );
    assert_eq!(
        parsed(&["serve"]).command,
        Command::Serve {
            host: None,
            port: None
        }
    );
}
