//! Famille `user`.

use crate::command::Command;
use crate::error::CliError;
use crate::globals::Arg;

use super::common::{Cursor, Token, usage};

pub(super) fn parse_user(args: &[Arg]) -> Result<Command, CliError> {
    let Some(first) = args.first() else {
        return Err(usage("user expects reset"));
    };
    if first.text != "reset" {
        return Err(usage(format!("unknown user command '{}'", first.text)));
    }
    let mut username = None;
    let mut cursor = Cursor::new(&args[1..]);
    while let Some(token) = cursor.token() {
        match token {
            Token::Option { name, inline } => match name {
                "--username" => username = Some(cursor.value("--username", inline)?),
                "--password" => {
                    return Err(usage(
                        "--password is not supported, enter the password when prompted",
                    ));
                }
                _ => return Err(usage(format!("unknown option '{name}'"))),
            },
            Token::Positional(_) => return Err(usage("user reset takes no positional arguments")),
        }
    }
    Ok(Command::UserReset { username })
}
