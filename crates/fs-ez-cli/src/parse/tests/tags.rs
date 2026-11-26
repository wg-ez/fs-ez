use super::*;

#[test]
fn tags_accept_inline_and_repeated_forms() {
    let inline = parsed(&["tag", "--tags=a,b,c", "one", "two"]);
    assert_eq!(
        inline.command,
        Command::Tag {
            tags: vec!["a".into(), "b".into(), "c".into()],
            paths: vec![PathBuf::from("one"), PathBuf::from("two")],
            force: false,
        }
    );
    let repeated = parsed(&["untag", "--tags", "a,b", "--tags", "c", "one"]);
    assert_eq!(
        repeated.command,
        Command::Untag {
            tags: vec!["a".into(), "b".into(), "c".into()],
            paths: vec![PathBuf::from("one")],
        }
    );
}

#[test]
fn tag_with_force() {
    let result = parsed(&["tag", "--force", "--tags=x", "path"]);
    assert!(matches!(result.command, Command::Tag { force: true, .. }));
}
