use super::*;

#[test]
fn usage_lists_every_group_and_command() {
    for group in ["Fichiers", "Tags", "Vues", "Inbox", "Auth", "Système"] {
        assert!(USAGE.contains(group), "groupe manquant {group}");
    }
    for command in [
        "init", "tag", "untag", "files", "tags", "rename", "merge", "delete", "repair", "status",
        "config", "serve", "group", "color", "view", "inbox", "token", "user", "help", "version",
    ] {
        assert!(
            USAGE.contains(&format!("fs-ez {command}")),
            "commande manquante {command}"
        );
    }
}

#[test]
fn usage_for_resolves_aliases_and_unknowns() {
    assert_eq!(usage_for("ls"), usage_for("files"));
    assert_eq!(usage_for("rm"), usage_for("delete"));
    assert_eq!(usage_for("nope"), USAGE);
}
