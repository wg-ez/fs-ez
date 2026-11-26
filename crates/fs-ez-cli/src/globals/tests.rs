use super::*;

fn owned(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_string()).collect()
}

#[test]
fn globals_before_after_and_middle() {
    let extracted = extract(&owned(&[
        "--json",
        "files",
        "--root",
        "/root",
        "alpha",
        "--quiet",
        "--db=/db.sqlite",
    ]))
    .expect("extraction");
    assert!(extracted.globals.json);
    assert!(extracted.globals.quiet);
    assert_eq!(extracted.globals.root, Some(PathBuf::from("/root")));
    assert_eq!(extracted.globals.db, Some(PathBuf::from("/db.sqlite")));
    let texts: Vec<&str> = extracted.args.iter().map(|arg| arg.text.as_str()).collect();
    assert_eq!(texts, ["files", "alpha"]);
}

#[test]
fn double_dash_keeps_rest_literal() {
    let extracted = extract(&owned(&["files", "--", "--json", "--root"])).expect("extraction");
    assert!(!extracted.globals.json);
    assert_eq!(extracted.args.len(), 3);
    assert!(extracted.args[1].literal);
    assert_eq!(extracted.args[1].text, "--json");
    assert!(extracted.args[2].literal);
}

#[test]
fn unknown_global_before_command_fails() {
    let error = extract(&owned(&["--nope", "files"])).expect_err("usage");
    assert!(matches!(error, CliError::Usage(_)));
}

#[test]
fn root_without_value_fails() {
    let error = extract(&owned(&["--root"])).expect_err("usage");
    assert!(matches!(error, CliError::Usage(_)));
}

#[test]
fn help_and_version_flags() {
    let extracted = extract(&owned(&["tag", "--help", "--version"])).expect("extraction");
    assert!(extracted.help);
    assert!(extracted.version);
}
