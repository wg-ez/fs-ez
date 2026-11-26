//! Famille `serve`.

use crate::command::Command;
use crate::error::CliError;
use crate::globals::Arg;

use super::common::{Cursor, Token, parse_number, usage};

pub(super) fn parse_serve(args: &[Arg]) -> Result<Command, CliError> {
    let mut host = None;
    let mut port = None;
    let mut cursor = Cursor::new(args);
    while let Some(token) = cursor.token() {
        match token {
            Token::Option { name, inline } => match name {
                "--host" => host = Some(cursor.value("--host", inline)?),
                "--port" => {
                    let value = cursor.value("--port", inline)?;
                    let parsed: u16 = parse_number(&value, "port")?;
                    if parsed == 0 {
                        return Err(usage(format!("invalid port '{value}'")));
                    }
                    port = Some(parsed);
                }
                _ => return Err(usage(format!("unknown option '{name}'"))),
            },
            Token::Positional(_) => return Err(usage("serve takes no positional arguments")),
        }
    }
    Ok(Command::Serve { host, port })
}
