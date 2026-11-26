//! Famille `token`.

use crate::command::Command;
use crate::error::CliError;
use crate::globals::Arg;

use super::common::{Cursor, ensure_empty, parse_token_id, reject_options, usage};

pub(super) fn parse_token(args: &[Arg]) -> Result<Command, CliError> {
    let Some(first) = args.first() else {
        return Err(usage("token expects add, list or revoke"));
    };
    let sub = first.text.as_str();
    let rest = &args[1..];
    match sub {
        "add" => {
            let mut expires = None;
            let mut cursor = Cursor::new(rest);
            let names = cursor.parse(|cursor, name, inline| {
                match name {
                    "--expires" => expires = Some(cursor.value("--expires", inline)?),
                    _ => return Err(usage(format!("unknown option '{name}'"))),
                }
                Ok(())
            })?;
            match names.as_slice() {
                [name] => Ok(Command::TokenAdd {
                    name: name.clone(),
                    expires,
                }),
                _ => Err(usage("token add expects exactly one name")),
            }
        }
        "list" => {
            ensure_empty(rest, "token list")?;
            Ok(Command::TokenList)
        }
        "revoke" => {
            let values = reject_options(rest)?;
            match values.as_slice() {
                [id] => Ok(Command::TokenRevoke {
                    id: parse_token_id(id)?,
                }),
                _ => Err(usage("token revoke expects exactly one id")),
            }
        }
        _ => Err(usage(format!("unknown token command '{sub}'"))),
    }
}
