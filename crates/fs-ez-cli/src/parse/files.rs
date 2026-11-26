//! Famille `files`.

use fs_ez_core::Sort;

use crate::command::Command;
use crate::error::CliError;
use crate::globals::Arg;

use super::common::{Cursor, parse_limit, parse_offset, parse_sort, usage};

pub(super) fn parse_files(args: &[Arg]) -> Result<Command, CliError> {
    let mut sort = Sort::default();
    let mut limit = 100;
    let mut offset = 0;
    let mut untagged = false;
    let mut recent = false;
    let mut print0 = false;
    let mut cursor = Cursor::new(args);
    let words = cursor.parse(|cursor, name, inline| {
        match name {
            "--sort" => {
                let value = cursor.value("--sort", inline)?;
                sort = parse_sort(&value)?;
            }
            "--limit" => {
                let value = cursor.value("--limit", inline)?;
                limit = parse_limit(&value)?;
            }
            "--offset" => {
                let value = cursor.value("--offset", inline)?;
                offset = parse_offset(&value)?;
            }
            "--untagged" if cursor.flag(inline) => untagged = true,
            "--recent" if cursor.flag(inline) => recent = true,
            "--print0" if cursor.flag(inline) => print0 = true,
            _ => return Err(usage(format!("unknown option '{name}'"))),
        }
        Ok(())
    })?;
    let joined = words.join(" ");
    let trimmed = joined.trim();
    let query = (!trimmed.is_empty()).then(|| trimmed.to_string());
    Ok(Command::Files {
        query,
        sort,
        limit,
        offset,
        untagged,
        recent,
        print0,
    })
}
