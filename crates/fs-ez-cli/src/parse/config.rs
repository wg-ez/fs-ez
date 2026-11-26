//! Famille `config`.

use crate::command::Command;
use crate::error::CliError;
use crate::globals::Arg;

use super::common::{reject_options, usage};

pub(super) fn parse_config(args: &[Arg]) -> Result<Command, CliError> {
    let values = reject_options(args)?;
    match values.as_slice() {
        [sub] if sub == "list" => Ok(Command::ConfigList),
        [sub, key] if sub == "get" => Ok(Command::ConfigGet { key: key.clone() }),
        [sub, key, value] if sub == "set" => Ok(Command::ConfigSet {
            key: key.clone(),
            value: value.clone(),
        }),
        _ => Err(usage("config expects list, get <key> or set <key> <value>")),
    }
}
