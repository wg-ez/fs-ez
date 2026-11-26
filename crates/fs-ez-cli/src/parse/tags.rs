//! Famille `tag`, `untag` et `merge`.

use std::path::PathBuf;

use crate::command::Command;
use crate::error::CliError;
use crate::globals::Arg;

use super::common::{Cursor, Token, push_tags, usage};

pub(super) fn parse_tag(args: &[Arg], allow_force: bool) -> Result<Command, CliError> {
    let mut tags = Vec::new();
    let mut paths = Vec::new();
    let mut force = false;
    let mut cursor = Cursor::new(args);
    while let Some(token) = cursor.token() {
        match token {
            Token::Option { name, inline } => match name {
                "--tags" => {
                    let value = cursor.value("--tags", inline)?;
                    push_tags(&mut tags, &value);
                }
                "--force" if allow_force && inline.is_none() => {
                    force = true;
                    cursor.skip();
                }
                _ => return Err(usage(format!("unknown option '{name}'"))),
            },
            Token::Positional(arg) => {
                paths.push(PathBuf::from(&arg.text));
                cursor.skip();
            }
        }
    }
    if tags.is_empty() {
        return Err(usage("--tags is required"));
    }
    if paths.is_empty() {
        return Err(usage("at least one path is required"));
    }
    if allow_force {
        Ok(Command::Tag { tags, paths, force })
    } else {
        Ok(Command::Untag { tags, paths })
    }
}

pub(super) fn parse_merge(args: &[Arg]) -> Result<Command, CliError> {
    let mut target = None;
    let mut sources = Vec::new();
    let mut cursor = Cursor::new(args);
    while let Some(token) = cursor.token() {
        match token {
            Token::Option { name, inline } => match name {
                "--into" => target = Some(cursor.value("--into", inline)?),
                _ => return Err(usage(format!("unknown option '{name}'"))),
            },
            Token::Positional(arg) => {
                sources.push(arg.text.clone());
                cursor.skip();
            }
        }
    }
    let target = target.ok_or_else(|| usage("--into is required"))?;
    if sources.is_empty() {
        return Err(usage("merge expects at least one source"));
    }
    Ok(Command::Merge { sources, target })
}
