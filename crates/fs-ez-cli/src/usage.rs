//! Aide et erreurs d'usage.

use crate::command::resolve_alias;

/// Aide d'une famille : nom, résumé ciblé, bloc de l'aide complète.
struct CommandHelp {
    family: &'static str,
    summary: Option<&'static str>,
    block: &'static str,
}

/// Source unique des commandes, `USAGE` étant concaténé à la compilation.
macro_rules! command_help {
    (
        $preamble:literal,
        $( $section:literal => [
            $( ($family:literal, $summary:expr, $block:literal $(,)?) ),* $(,)?
        ] ),* $(,)?
    ) => {
        pub const USAGE: &str = concat!(
            $preamble,
            $(
                $section, "\n",
                $( $block ),*
            ),*
        );

        const COMMAND_HELP: &[CommandHelp] = &[
            $(
                $(
                    CommandHelp {
                        family: $family,
                        summary: $summary,
                        block: $block,
                    }
                ),*
            ),*
        ];
    };
}

command_help! {
    "usage: fs-ez [global options] <command> [options]

Global options
  --root <path>        root directory
  --db <path>          SQLite database
  --json               machine-readable output
  --quiet              suppress non-essential output
  --verbose            verbose diagnostics
  --no-color           disable colors
  --yes                non-interactive confirmation
",
    "\nFichiers" => [
        ("init", None, "  fs-ez init\n"),
        ("tag", None, "  fs-ez tag --tags=<t1,t2> [--force] <path>...\n"),
        ("untag", None, "  fs-ez untag --tags=<t1,t2> <path>...\n"),
        ("files", None, "  fs-ez files [query] [--sort=<id|name|size|mtime>[:asc|desc]] [--limit=<n>] [--offset=<n>] [--untagged] [--recent] [--print0]\n"),
        ("delete", None, "  fs-ez delete <name>...\n"),
        ("repair", None, "  fs-ez repair\n"),
        ("status", None, "  fs-ez status\n"),
    ],
    "\nTags" => [
        ("tags", None, "  fs-ez tags\n"),
        ("rename", None, "  fs-ez rename <old> <new>\n"),
        ("merge", None, "  fs-ez merge --into=<target> <source>...\n"),
        ("color", None, "  fs-ez color set <tag> <token>\n"),
        (
            "group",
            Some("  fs-ez group list | add <name> [--color=<token>] | rm <name> | assign <group> <tag> | move <from> <to> <tag>"),
            "  fs-ez group list\n  fs-ez group add <name> [--color=<token>]\n  fs-ez group rm <name>\n  fs-ez group assign <group> <tag>\n  fs-ez group move <from> <to> <tag>\n",
        ),
    ],
    "\nVues" => [
        (
            "view",
            Some("  fs-ez view list | add <name> [options] [--group=<name>] | rm <name> | run <name> [--limit=<n>] [--offset=<n>] [--print0]"),
            "  fs-ez view list\n  fs-ez view add <name> [--query=<q>] [--filter=<json>] [--icon=<i>] [--color=<token>] [--sort=<id|name|size|mtime>[:asc|desc]] [--group=<name>]\n  fs-ez view rm <name>\n  fs-ez view run <name> [--limit=<n>] [--offset=<n>] [--print0]\n",
        ),
        (
            "view-group",
            Some("  fs-ez view-group list | add <name> [--color=<token>] | rm <name> | assign <group> <view> | move <from> <to> <view>"),
            "  fs-ez view-group list\n  fs-ez view-group add <name> [--color=<token>]\n  fs-ez view-group rm <name>\n  fs-ez view-group assign <group> <view>\n  fs-ez view-group move <from> <to> <view>\n",
        ),
    ],
    "\nInbox" => [
        ("inbox", None, "  fs-ez inbox scan\n"),
    ],
    "\nAuth" => [
        (
            "token",
            Some("  fs-ez token add <name> [--expires=<rfc3339>] | list | revoke <id>"),
            "  fs-ez token add <name> [--expires=<rfc3339>]\n  fs-ez token list\n  fs-ez token revoke <id>\n",
        ),
        ("user", None, "  fs-ez user reset [--username=<name>]\n"),
    ],
    "\nSystème" => [
        (
            "config",
            Some("  fs-ez config list | get <key> | set <key> <value>"),
            "  fs-ez config list\n  fs-ez config get <key>\n  fs-ez config set <key> <value>\n",
        ),
        ("serve", None, "  fs-ez serve [--host=<host>] [--port=<port>]\n"),
        ("help", None, "  fs-ez help [command]\n"),
        ("version", None, "  fs-ez version\n"),
    ],
}

/// Aide ciblée ; commande inconnue = aide complète.
pub fn usage_for(command: &str) -> &'static str {
    let family = resolve_alias(command);
    COMMAND_HELP
        .iter()
        .find(|entry| entry.family == family)
        .map(|entry| entry.summary.unwrap_or_else(|| first_line(entry.block)))
        .unwrap_or(USAGE)
}

fn first_line(block: &'static str) -> &'static str {
    block.lines().next().unwrap_or(block)
}

#[cfg(test)]
mod tests;
