//! Famille `group` et `color`.

use crate::command::Command;
use crate::error::CliError;
use crate::globals::Arg;

use super::common::{ensure_empty, exact_args, parse_name_color, reject_options, usage};

pub(super) fn parse_group(args: &[Arg]) -> Result<Command, CliError> {
    let Some(first) = args.first() else {
        return Err(usage("group expects list, add, rm, assign or move"));
    };
    let sub = first.text.as_str();
    let rest = &args[1..];
    match sub {
        "list" => {
            ensure_empty(rest, "group list")?;
            Ok(Command::GroupList)
        }
        "add" => {
            let (name, color) = parse_name_color(rest, "group")?;
            Ok(Command::GroupAdd { name, color })
        }
        "rm" => {
            let [name] = exact_args::<1>(rest, "group rm expects exactly one name")?;
            Ok(Command::GroupRm { name })
        }
        "assign" => {
            let [group, tag] = exact_args::<2>(rest, "group assign expects a group and a tag")?;
            Ok(Command::GroupAssign { group, tag })
        }
        "move" => {
            let [from, to, tag] =
                exact_args::<3>(rest, "group move expects a source, a target and a tag")?;
            Ok(Command::GroupMove { from, to, tag })
        }
        _ => Err(usage(format!("unknown group command '{sub}'"))),
    }
}

pub(super) fn parse_color(args: &[Arg]) -> Result<Command, CliError> {
    let values = reject_options(args)?;
    match values.as_slice() {
        [sub, tag, token] if sub == "set" => Ok(Command::ColorSet {
            tag: tag.clone(),
            token: token.clone(),
        }),
        _ => Err(usage("color expects set <tag> <token>")),
    }
}
