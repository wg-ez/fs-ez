use super::*;

#[test]
fn globals_can_appear_anywhere() {
    let result = parsed(&["--json", "files", "--quiet", "alpha", "--root=/tmp/x"]);
    assert!(result.globals.json);
    assert!(result.globals.quiet);
    assert_eq!(result.globals.root, Some(PathBuf::from("/tmp/x")));
    assert!(matches!(
        result.command,
        Command::Files { query: Some(ref query), .. } if query == "alpha"
    ));
}

#[test]
fn double_dash_stops_global_extraction() {
    let result = parsed(&["files", "--", "--json"]);
    assert!(!result.globals.json);
    assert!(matches!(
        result.command,
        Command::Files { query: Some(ref query), .. } if query == "--json"
    ));
}
