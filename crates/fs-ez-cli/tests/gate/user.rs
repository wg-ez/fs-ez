use super::*;

#[test]
fn tokens_and_user_reset() {
    let fixture = CliFixture::new();
    fixture.ok(&["init"]);

    let created = fixture.json(&["token", "add", "ci", "--expires=2030-01-01T00:00:00Z"]);
    let token = created["token"].as_str().expect("jeton");
    assert_eq!(token.chars().count(), 43);
    let id = created["id"].as_i64().expect("id").to_string();

    let listed = fixture.json(&["token", "list"]);
    let tokens = listed["tokens"].as_array().expect("tokens");
    assert_eq!(tokens.len(), 1);
    assert!(tokens[0].get("token").is_none());

    let revoked = fixture.json(&["token", "revoke", &id]);
    assert_eq!(revoked["ok"], json!(true));
    assert_eq!(
        fixture.json(&["token", "list"])["tokens"]
            .as_array()
            .expect("tokens")
            .len(),
        0
    );

    let mut command = fixture.command();
    command.write_stdin("correct horse battery\n");
    let run = fixture.run_with(
        &mut command,
        &["user", "reset", "--username=root", "--json"],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    let reset: Value = serde_json::from_str(&run.stdout).expect("JSON valide sur stdout");
    assert_eq!(reset["schema"], json!(1));
    assert_eq!(reset["ok"], json!(true));
    assert!(reset.get("password").is_none());
    assert!(reset.get("password_hash").is_none());
}
