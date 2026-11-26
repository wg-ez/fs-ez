//! Helpers partagés : résolution d'entités, conversion JSON, chemins et mot de passe.

use std::io::{BufRead, IsTerminal, Write};

use fs_ez_app::AppError;
use fs_ez_core::{FileWithTags, SavedView, TagColor, TagGroup, ViewGroup};
use fs_ez_store::Page;
use serde::Serialize;
use serde_json::{Value as Json, json};

use crate::context::CliContext;
use crate::error::CliError;
use crate::output::Output;
use crate::render;

pub(super) fn files_output(files: Vec<FileWithTags>) -> Result<Output, CliError> {
    Ok(Output::new(
        render::file_table(&files),
        json!({"files": to_json(&files)?}),
    ))
}

pub(super) fn ensure_print0_allowed(json: bool, print0: bool) -> Result<(), CliError> {
    if print0 && json {
        return Err(CliError::Usage(
            "--print0 cannot be combined with --json".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn files_page_output(
    page: Page<FileWithTags>,
    print0: bool,
) -> Result<Output, CliError> {
    let payload = json!({
        "items": to_json(&page.items)?,
        "next_offset": page.next_offset,
    });
    if print0 {
        let mut human = String::new();
        for item in &page.items {
            human.push_str(&item.file.path_display);
            human.push('\0');
        }
        return Ok(Output::new(human, payload));
    }
    Ok(Output::new(render::file_table(&page.items), payload))
}

/// JSON d'un groupe, enfants insérés sous `children_key`.
pub(super) fn group_json<G: Serialize, C: Serialize>(
    group: &G,
    children: &[C],
    children_key: &str,
) -> Result<Json, CliError> {
    let mut value = to_json(group)?;
    if let Json::Object(object) = &mut value {
        object.insert(children_key.to_string(), to_json(&children)?);
    }
    Ok(value)
}

pub(super) fn parse_color_token(raw: &str) -> Result<TagColor, CliError> {
    TagColor::parse(raw).ok_or_else(|| {
        let tokens: Vec<&str> = TagColor::ALL.iter().map(TagColor::as_token).collect();
        CliError::Usage(format!(
            "unknown color token '{raw}', expected one of: {}",
            tokens.join(", ")
        ))
    })
}

/// Premier élément dont `key` égale `name`, message `label name` sinon.
fn find_by_name<T>(
    items: Vec<T>,
    name: &str,
    label: &str,
    key: impl Fn(&T) -> &str,
) -> Result<T, CliError> {
    items
        .into_iter()
        .find(|item| key(item) == name)
        .ok_or_else(|| AppError::NotFound(format!("{label} {name}")).into())
}

pub(super) fn resolve_group(ctx: &CliContext, name: &str) -> Result<TagGroup, CliError> {
    find_by_name(ctx.app.list_groups()?, name, "group", |group| {
        group.name.as_str()
    })
}

pub(super) fn resolve_view(ctx: &CliContext, name: &str) -> Result<SavedView, CliError> {
    find_by_name(ctx.app.list_views()?, name, "view", |view| {
        view.name.as_str()
    })
}

pub(super) fn resolve_view_group(ctx: &CliContext, name: &str) -> Result<ViewGroup, CliError> {
    find_by_name(ctx.app.list_view_groups()?, name, "view group", |group| {
        group.name.as_str()
    })
}

pub(super) fn read_password() -> Result<String, CliError> {
    let stdin = std::io::stdin();
    let interactive = stdin.is_terminal();
    read_password_from(&mut stdin.lock(), interactive)
}

pub(super) fn read_password_from<R: BufRead>(
    reader: &mut R,
    interactive: bool,
) -> Result<String, CliError> {
    if interactive {
        eprint!("password: ");
        std::io::stderr()
            .flush()
            .map_err(|error| CliError::Io(error.to_string()))?;
    }
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|error| CliError::Io(error.to_string()))?;
    while line.ends_with('\n') || line.ends_with('\r') {
        line.pop();
    }
    Ok(line)
}

pub(super) fn to_json<T: Serialize>(value: &T) -> Result<Json, CliError> {
    serde_json::to_value(value).map_err(|error| CliError::Json(error.to_string()))
}

#[cfg(test)]
mod tests;
