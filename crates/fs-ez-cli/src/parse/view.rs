//! Famille `view`.

use fs_ez_core::Sort;

use crate::command::Command;
use crate::error::CliError;
use crate::globals::Arg;

use super::common::{
    Cursor, ensure_empty, parse_limit, parse_offset, parse_sort, reject_options, usage,
};

pub(super) fn parse_view(args: &[Arg]) -> Result<Command, CliError> {
    let Some(first) = args.first() else {
        return Err(usage("view expects list, add, rm or run"));
    };
    let sub = first.text.as_str();
    let rest = &args[1..];
    match sub {
        "list" => {
            ensure_empty(rest, "view list")?;
            Ok(Command::ViewList)
        }
        "add" => parse_view_add(rest),
        "rm" => {
            let values = reject_options(rest)?;
            match values.as_slice() {
                [name] => Ok(Command::ViewRm { name: name.clone() }),
                _ => Err(usage("view rm expects exactly one name")),
            }
        }
        "run" => parse_view_run(rest),
        _ => Err(usage(format!("unknown view command '{sub}'"))),
    }
}

fn parse_view_add(args: &[Arg]) -> Result<Command, CliError> {
    let mut query = None;
    let mut filter = None;
    let mut icon = None;
    let mut color = None;
    let mut group = None;
    let mut sort = Sort::default();
    let mut cursor = Cursor::new(args);
    let names = cursor.parse(|cursor, name, inline| {
        match name {
            "--query" => query = Some(cursor.value("--query", inline)?),
            "--filter" => filter = Some(cursor.value("--filter", inline)?),
            "--icon" => icon = Some(cursor.value("--icon", inline)?),
            "--color" => color = Some(cursor.value("--color", inline)?),
            "--group" => group = Some(cursor.value("--group", inline)?),
            "--sort" => {
                let value = cursor.value("--sort", inline)?;
                sort = parse_sort(&value)?;
            }
            _ => return Err(usage(format!("unknown option '{name}'"))),
        }
        Ok(())
    })?;
    match names.as_slice() {
        [name] => Ok(Command::ViewAdd {
            name: name.clone(),
            query,
            filter,
            icon,
            color,
            sort,
            group,
        }),
        _ => Err(usage("view add expects exactly one name")),
    }
}

fn parse_view_run(args: &[Arg]) -> Result<Command, CliError> {
    let mut limit = 100;
    let mut offset = 0;
    let mut print0 = false;
    let mut cursor = Cursor::new(args);
    let names = cursor.parse(|cursor, name, inline| {
        match name {
            "--limit" => {
                let value = cursor.value("--limit", inline)?;
                limit = parse_limit(&value)?;
            }
            "--offset" => {
                let value = cursor.value("--offset", inline)?;
                offset = parse_offset(&value)?;
            }
            "--print0" if cursor.flag(inline) => print0 = true,
            _ => return Err(usage(format!("unknown option '{name}'"))),
        }
        Ok(())
    })?;
    match names.as_slice() {
        [name] => Ok(Command::ViewRun {
            name: name.clone(),
            limit,
            offset,
            print0,
        }),
        _ => Err(usage("view run expects exactly one name")),
    }
}
