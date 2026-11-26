//! Famille `inbox`.

use crate::command::Command;
use crate::error::CliError;
use crate::globals::Arg;

use super::common::{ensure_empty, usage};

pub(super) fn parse_inbox(args: &[Arg]) -> Result<Command, CliError> {
    let Some(first) = args.first() else {
        return Err(usage("inbox expects scan"));
    };
    match first.text.as_str() {
        "scan" => {
            ensure_empty(&args[1..], "inbox scan")?;
            Ok(Command::InboxScan)
        }
        sub => Err(usage(format!("unknown inbox command '{sub}'"))),
    }
}
