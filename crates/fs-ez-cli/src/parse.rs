//! Parsing maison des arguments, sans dépendance de type `clap`.

use crate::command::{Command, resolve_alias};
use crate::error::CliError;
use crate::globals::{self, Extracted, GlobalOptions};

mod common;
mod config;
mod files;
mod group;
mod inbox;
mod serve;
mod tags;
mod token;
mod user;
mod view;
mod view_group;

use common::{ensure_empty, reject_options, usage};

// Ré-exports visibles des tests du module, qui font `use super::*`.
#[cfg(test)]
pub(crate) use fs_ez_core::{ApiTokenId, Sort, SortDir, SortField};
#[cfg(test)]
pub(crate) use std::path::PathBuf;

#[derive(Debug)]
pub struct Parsed {
    pub globals: GlobalOptions,
    pub command: Command,
}

pub fn parse(args: &[String]) -> Result<Parsed, CliError> {
    let extracted = globals::extract(args)?;
    let command = build(&extracted)?;
    Ok(Parsed {
        globals: extracted.globals,
        command,
    })
}

fn build(extracted: &Extracted) -> Result<Command, CliError> {
    if extracted.version {
        return Ok(Command::Version);
    }
    let Some(first) = extracted.args.first() else {
        return if extracted.help {
            Ok(Command::Help(None))
        } else {
            Err(usage("missing command"))
        };
    };
    let name = resolve_alias(&first.text);
    let args = &extracted.args[1..];
    if extracted.help {
        return Ok(Command::Help(Some(name.to_string())));
    }
    match name {
        "init" => {
            ensure_empty(args, "init")?;
            Ok(Command::Init)
        }
        "tag" => tags::parse_tag(args, true),
        "untag" => tags::parse_tag(args, false),
        "files" => files::parse_files(args),
        "tags" => {
            ensure_empty(args, "tags")?;
            Ok(Command::Tags)
        }
        "rename" => {
            let values = reject_options(args)?;
            match values.as_slice() {
                [old, new] => Ok(Command::Rename {
                    old: old.clone(),
                    new: new.clone(),
                }),
                _ => Err(usage("rename expects exactly two tags")),
            }
        }
        "merge" => tags::parse_merge(args),
        "delete" => {
            let names = reject_options(args)?;
            if names.is_empty() {
                return Err(usage("delete expects at least one name"));
            }
            Ok(Command::Delete { names })
        }
        "repair" => {
            ensure_empty(args, "repair")?;
            Ok(Command::Repair)
        }
        "status" => {
            ensure_empty(args, "status")?;
            Ok(Command::Status)
        }
        "config" => config::parse_config(args),
        "serve" => serve::parse_serve(args),
        "group" => group::parse_group(args),
        "color" => group::parse_color(args),
        "view" => view::parse_view(args),
        "view-group" => view_group::parse_view_group(args),
        "inbox" => inbox::parse_inbox(args),
        "token" => token::parse_token(args),
        "user" => user::parse_user(args),
        "help" => {
            let values = reject_options(args)?;
            match values.as_slice() {
                [] => Ok(Command::Help(None)),
                [command] => Ok(Command::Help(Some(command.clone()))),
                _ => Err(usage("help expects at most one command")),
            }
        }
        "version" => {
            ensure_empty(args, "version")?;
            Ok(Command::Version)
        }
        other => Err(usage(format!("unknown command '{other}'"))),
    }
}

#[cfg(test)]
mod tests;
