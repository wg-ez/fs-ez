//! Commandes des tags : application, renommage, fusion, suppression.

use std::path::PathBuf;

use fs_ez_core::display_safe;
use serde_json::json;

use crate::context::CliContext;
use crate::error::CliError;
use crate::output::Output;
use crate::render;

use super::common::{files_output, to_json};

pub(super) fn tag(
    ctx: &CliContext,
    tags: &[String],
    paths: &[PathBuf],
    force: bool,
) -> Result<Output, CliError> {
    files_output(ctx.app.tag_paths(paths, tags, force)?)
}

pub(super) fn untag(
    ctx: &CliContext,
    tags: &[String],
    paths: &[PathBuf],
) -> Result<Output, CliError> {
    files_output(ctx.app.untag_paths(paths, tags)?)
}

pub(super) fn list(ctx: &CliContext) -> Result<Output, CliError> {
    let tags = ctx.app.list_tags()?;
    let items: Vec<serde_json::Value> = tags
        .iter()
        .map(|item| {
            json!({
                "id": item.tag.id,
                "name": item.tag.name,
                "name_norm": item.tag.name_norm,
                "color": item.tag.color.as_token(),
                "count": item.count,
            })
        })
        .collect();
    Ok(Output::new(
        render::tag_table(&tags),
        json!({"tags": items}),
    ))
}

pub(super) fn rename(ctx: &CliContext, old: &str, new: &str) -> Result<Output, CliError> {
    let tag = ctx.app.rename_tag(old, new)?;
    Ok(Output::new(
        format!("renamed tag to {}\n", display_safe(&tag.name)),
        json!({"tag": to_json(&tag)?}),
    ))
}

pub(super) fn merge(
    ctx: &CliContext,
    sources: &[String],
    target: &str,
) -> Result<Output, CliError> {
    let files = ctx.app.merge_tags(sources, target)?;
    Ok(Output::new(
        format!("merged {files} file(s)\n"),
        json!({"ok": true, "files": files}),
    ))
}

pub(super) fn delete(ctx: &CliContext, names: &[String]) -> Result<Output, CliError> {
    let mut deleted = 0u64;
    for name in names {
        if ctx.app.delete_tag(name)? {
            deleted = deleted.saturating_add(1);
        }
    }
    Ok(Output::new(
        format!("deleted {deleted} tag(s)\n"),
        json!({"ok": true, "deleted": deleted}),
    ))
}
