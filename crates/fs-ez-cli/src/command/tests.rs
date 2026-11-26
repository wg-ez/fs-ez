use super::*;

#[test]
fn aliases_resolve_to_canonical_commands() {
    assert_eq!(resolve_alias("ls"), "files");
    assert_eq!(resolve_alias("find"), "files");
    assert_eq!(resolve_alias("rm"), "delete");
    assert_eq!(resolve_alias("tag"), "tag");
}
