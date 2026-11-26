use super::*;

#[test]
fn every_command_returns_versioned_json() {
    let fixture = CliFixture::new();
    fixture.ok(&["init"]);
    let path = fixture.write("contract.txt", b"contract");
    let arg = path.to_str().expect("utf8");
    fixture.write("files/contract.txt", b"inbox");
    fixture.ok(&["tag", "--tags=rust,sql,temp", arg]);

    let cases: Vec<Vec<&str>> = vec![
        vec!["init"],
        vec!["tag", "--tags=extra", arg],
        vec!["untag", "--tags=extra", arg],
        vec!["files"],
        vec!["tags"],
        vec!["rename", "sql", "database"],
        vec!["merge", "--into=rust", "database"],
        vec!["delete", "temp"],
        vec!["repair"],
        vec!["status"],
        vec!["config", "list"],
        vec!["config", "get", "ui.recent_days"],
        vec!["config", "set", "ui.recent_days", "3"],
        vec!["group", "list"],
        vec!["group", "add", "g1"],
        vec!["group", "assign", "g1", "rust"],
        vec!["group", "move", "g1", "g1", "rust"],
        vec!["color", "set", "rust", "green"],
        vec!["group", "rm", "g1"],
        vec!["view", "list"],
        vec!["view", "add", "V", "--query", "rust"],
        vec!["view", "run", "V"],
        vec!["view", "rm", "V"],
        vec!["inbox", "scan"],
        vec!["token", "add", "ci"],
        vec!["token", "list"],
        vec!["help"],
        vec!["version"],
    ];
    for case in cases {
        let value = fixture.json(&case);
        assert_eq!(value["schema"], json!(1), "commande {case:?}");
    }

    let tokens = fixture.json(&["token", "list"]);
    let token_id = tokens["tokens"][0]["id"]
        .as_i64()
        .expect("token id")
        .to_string();
    assert_eq!(
        fixture.json(&["token", "revoke", &token_id])["schema"],
        json!(1)
    );
    let mut command = fixture.command();
    command.write_stdin("correct horse battery\n");
    let run = fixture.run_with(&mut command, &["--json", "user", "reset"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let value: Value = serde_json::from_str(&run.stdout).expect("JSON valide sur stdout");
    assert_eq!(value["schema"], json!(1));
}

#[test]
fn usage_errors_go_to_stderr_with_code_one() {
    let fixture = CliFixture::new();
    fixture.ok(&["init"]);
    let outside = fixture.outside("outside.txt", b"outside");

    let cases: Vec<Vec<&str>> = vec![
        vec!["commande-inconnue"],
        vec!["tag", "sans-tags"],
        vec!["tag", "--tags=x", outside.to_str().expect("utf8")],
        vec!["files", "--sort=invalide"],
    ];
    for args in cases {
        let run = fixture.run(&args);
        assert_eq!(run.code, 1, "code pour {args:?}");
        assert!(
            run.stdout.is_empty(),
            "stdout pour {args:?}: {}",
            run.stdout
        );
        assert!(!run.stderr.is_empty(), "stderr vide pour {args:?}");
    }

    let bogus = fixture
        .command()
        .arg("bogus")
        .output()
        .expect("exécution bogus");
    // `cargo run -p fs-ez -- bogus` doit sortir en erreur : même binaire, même code.
    assert_eq!(bogus.status.code(), Some(1));
}
