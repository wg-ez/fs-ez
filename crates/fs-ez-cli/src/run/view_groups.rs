//! Commandes des groupes de vues.

use fs_ez_core::display_safe;
use serde_json::json;

use crate::context::CliContext;
use crate::error::CliError;
use crate::output::Output;
use crate::render;

use super::common::{group_json, parse_color_token, resolve_view, resolve_view_group};

pub(super) fn list(ctx: &CliContext) -> Result<Output, CliError> {
    let groups = ctx.app.view_groups_with_views()?;
    let items = groups
        .iter()
        .map(|(group, views)| group_json(group, views, "views"))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Output::new(
        render::view_group_table(&groups),
        json!({"view_groups": items}),
    ))
}

pub(super) fn add(ctx: &CliContext, name: &str, color: Option<&str>) -> Result<Output, CliError> {
    let color = color.map(parse_color_token).transpose()?;
    let group = ctx.app.create_view_group(name, color)?;
    let value = group_json(&group, &[] as &[fs_ez_core::SavedView], "views")?;
    Ok(Output::new(
        render::view_group_table(&[(group, Vec::new())]),
        json!({"view_group": value}),
    ))
}

pub(super) fn remove(ctx: &CliContext, name: &str) -> Result<Output, CliError> {
    let group = resolve_view_group(ctx, name)?;
    ctx.app.delete_view_group(group.id)?;
    Ok(Output::new(
        format!("deleted view group {}\n", display_safe(&group.name)),
        json!({"ok": true}),
    ))
}

pub(super) fn assign(ctx: &CliContext, group: &str, view: &str) -> Result<Output, CliError> {
    let group = resolve_view_group(ctx, group)?;
    let view = resolve_view(ctx, view)?;
    ctx.app.assign_view(group.id, view.id)?;
    Ok(Output::new(
        format!(
            "assigned {} to {}\n",
            display_safe(&view.name),
            display_safe(&group.name)
        ),
        json!({"ok": true}),
    ))
}

pub(super) fn move_view(
    ctx: &CliContext,
    from: &str,
    to: &str,
    view: &str,
) -> Result<Output, CliError> {
    let from = resolve_view_group(ctx, from)?;
    let to = resolve_view_group(ctx, to)?;
    let view = resolve_view(ctx, view)?;
    ctx.app.move_view(from.id, to.id, view.id)?;
    Ok(Output::new(
        format!(
            "moved {} from {} to {}\n",
            display_safe(&view.name),
            display_safe(&from.name),
            display_safe(&to.name)
        ),
        json!({"ok": true}),
    ))
}
