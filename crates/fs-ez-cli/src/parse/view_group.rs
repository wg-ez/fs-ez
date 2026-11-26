//! Famille `view-group`.

use crate::command::Command;
use crate::error::CliError;
use crate::globals::Arg;

use super::common::{ensure_empty, exact_args, parse_name_color, usage};

pub(super) fn parse_view_group(args: &[Arg]) -> Result<Command, CliError> {
    let Some(first) = args.first() else {
        return Err(usage("view-group expects list, add, rm, assign or move"));
    };
    let sub = first.text.as_str();
    let rest = &args[1..];
    match sub {
        "list" => {
            ensure_empty(rest, "view-group list")?;
            Ok(Command::ViewGroupList)
        }
        "add" => {
            let (name, color) = parse_name_color(rest, "view-group")?;
            Ok(Command::ViewGroupAdd { name, color })
        }
        "rm" => {
            let [name] = exact_args::<1>(rest, "view-group rm expects exactly one name")?;
            Ok(Command::ViewGroupRm { name })
        }
        "assign" => {
            let [group, view] =
                exact_args::<2>(rest, "view-group assign expects a group and a view")?;
            Ok(Command::ViewGroupAssign { group, view })
        }
        "move" => {
            let [from, to, view] = exact_args::<3>(
                rest,
                "view-group move expects a source, a target and a view",
            )?;
            Ok(Command::ViewGroupMove { from, to, view })
        }
        _ => Err(usage(format!("unknown view-group command '{sub}'"))),
    }
}
