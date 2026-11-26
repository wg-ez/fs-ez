use super::*;

#[test]
fn inbox_scan_is_the_only_variant() {
    assert_eq!(parsed(&["inbox", "scan"]).command, Command::InboxScan);
    assert!(matches!(failed(&["inbox", "list"]), CliError::Usage(_)));
    assert!(matches!(
        failed(&["inbox", "archive", "9"]),
        CliError::Usage(_)
    ));
}

#[test]
fn token_variants() {
    assert_eq!(
        parsed(&["token", "add", "ci", "--expires=2030-01-01T00:00:00Z"]).command,
        Command::TokenAdd {
            name: "ci".into(),
            expires: Some("2030-01-01T00:00:00Z".into())
        }
    );
    assert_eq!(
        parsed(&["token", "revoke", "12"]).command,
        Command::TokenRevoke { id: ApiTokenId(12) }
    );
    for id in ["0", "-1"] {
        assert!(matches!(
            failed(&["token", "revoke", id]),
            CliError::Usage(_)
        ));
    }
    assert_eq!(
        parsed(&["user", "reset", "--username=root"]).command,
        Command::UserReset {
            username: Some("root".into())
        }
    );
    assert_eq!(
        parsed(&["user", "reset"]).command,
        Command::UserReset { username: None }
    );
}

#[test]
fn user_reset_rejects_password_option() {
    for args in [
        &["user", "reset", "--password=secret"][..],
        &["user", "reset", "--password", "secret"][..],
    ] {
        assert!(
            matches!(failed(args), CliError::Usage(message) if message.contains("--password")),
            "mot de passe refusé dans argv: {args:?}"
        );
    }
}
