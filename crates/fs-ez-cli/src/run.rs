//! Exécution des commandes de base sur un contexte applicatif déjà construit.

mod common;
mod config;
mod files;
mod groups;
mod inbox;
mod system;
mod tags;
mod tokens;
mod user;
mod view_groups;
mod views;

use serde_json::json;

use crate::command::Command;
use crate::context::CliContext;
use crate::error::CliError;
use crate::output::Output;
use crate::parse::Parsed;
use crate::usage::{USAGE, usage_for};

pub enum Outcome {
    Output(Output),
    /// Le binaire lit `PRAGMA user_version`, `fs-ez-cli` ne connaît pas SQLite.
    Version,
    Serve {
        host: Option<String>,
        port: Option<u16>,
    },
}

pub fn execute(parsed: &Parsed, ctx: &CliContext) -> Result<Outcome, CliError> {
    match &parsed.command {
        Command::Help(name) => Ok(Outcome::Output(help_output(name.as_deref()))),
        Command::Version => Ok(Outcome::Version),
        Command::Serve { host, port } => Ok(Outcome::Serve {
            host: host.clone(),
            port: *port,
        }),
        command => execute_output(command, ctx).map(Outcome::Output),
    }
}

fn execute_output(command: &Command, ctx: &CliContext) -> Result<Output, CliError> {
    match command {
        Command::Init => system::init(ctx),
        Command::Tag { tags, paths, force } => tags::tag(ctx, tags, paths, *force),
        Command::Untag { tags, paths } => tags::untag(ctx, tags, paths),
        Command::Files {
            query,
            sort,
            limit,
            offset,
            untagged,
            recent,
            print0,
        } => files::files(
            ctx,
            query.as_deref(),
            *sort,
            *limit,
            *offset,
            *untagged,
            *recent,
            *print0,
        ),
        Command::Tags => tags::list(ctx),
        Command::Rename { old, new } => tags::rename(ctx, old, new),
        Command::Merge { sources, target } => tags::merge(ctx, sources, target),
        Command::Delete { names } => tags::delete(ctx, names),
        Command::Repair => system::repair(ctx),
        Command::Status => system::status(ctx),
        Command::ConfigList => config::list(ctx),
        Command::ConfigGet { key } => config::get(ctx, key),
        Command::ConfigSet { key, value } => config::set(ctx, key, value),
        Command::GroupList => groups::list(ctx),
        Command::GroupAdd { name, color } => groups::add(ctx, name, color.as_deref()),
        Command::GroupRm { name } => groups::remove(ctx, name),
        Command::GroupAssign { group, tag } => groups::assign(ctx, group, tag),
        Command::GroupMove { from, to, tag } => groups::move_tag(ctx, from, to, tag),
        Command::ColorSet { tag, token } => groups::color_set(ctx, tag, token),
        Command::ViewList => views::list(ctx),
        Command::ViewAdd {
            name,
            query,
            filter,
            icon,
            color,
            sort,
            group,
        } => views::add(
            ctx,
            name,
            query.as_deref(),
            filter.as_deref(),
            icon.as_deref(),
            color.as_deref(),
            *sort,
            group.as_deref(),
        ),
        Command::ViewRm { name } => views::remove(ctx, name),
        Command::ViewRun {
            name,
            limit,
            offset,
            print0,
        } => views::run(ctx, name, *limit, *offset, *print0),
        Command::ViewGroupList => view_groups::list(ctx),
        Command::ViewGroupAdd { name, color } => view_groups::add(ctx, name, color.as_deref()),
        Command::ViewGroupRm { name } => view_groups::remove(ctx, name),
        Command::ViewGroupAssign { group, view } => view_groups::assign(ctx, group, view),
        Command::ViewGroupMove { from, to, view } => view_groups::move_view(ctx, from, to, view),
        Command::InboxScan => inbox::scan(ctx),
        Command::TokenAdd { name, expires } => tokens::add(ctx, name, expires.as_deref()),
        Command::TokenList => tokens::list(ctx),
        Command::TokenRevoke { id } => tokens::revoke(ctx, *id),
        Command::UserReset { username } => user::reset(ctx, username.as_deref()),
        Command::Help(_) | Command::Version | Command::Serve { .. } => {
            Err(CliError::Usage("command handled elsewhere".to_string()))
        }
    }
}

fn help_output(name: Option<&str>) -> Output {
    let usage = match name {
        Some(command) => usage_for(command),
        None => USAGE,
    };
    let human = if usage.ends_with('\n') {
        usage.to_string()
    } else {
        format!("{usage}\n")
    };
    Output::new(human, json!({"usage": usage}))
}

#[cfg(test)]
mod tests;
