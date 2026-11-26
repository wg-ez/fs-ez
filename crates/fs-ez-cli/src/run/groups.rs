//! Commandes des groupes de tags et de la couleur d'un tag.

use fs_ez_core::display_safe;
use serde_json::json;

use crate::context::CliContext;
use crate::error::CliError;
use crate::output::Output;
use crate::render;

use super::common::{group_json, parse_color_token, resolve_group, to_json};

pub(super) fn list(ctx: &CliContext) -> Result<Output, CliError> {
    let groups = ctx.app.groups_with_tags()?;
    let items = groups
        .iter()
        .map(|(group, tags)| group_json(group, tags, "tags"))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Output::new(
        render::group_table(&groups),
        json!({"groups": items}),
    ))
}

pub(super) fn add(ctx: &CliContext, name: &str, color: Option<&str>) -> Result<Output, CliError> {
    let color = color.map(parse_color_token).transpose()?;
    let group = ctx.app.create_group(name, color)?;
    let value = group_json(&group, &[] as &[fs_ez_core::Tag], "tags")?;
    Ok(Output::new(
        render::group_table(&[(group, Vec::new())]),
        json!({"group": value}),
    ))
}

pub(super) fn remove(ctx: &CliContext, name: &str) -> Result<Output, CliError> {
    let group = resolve_group(ctx, name)?;
    ctx.app.delete_group(group.id)?;
    Ok(Output::new(
        format!("deleted group {}\n", display_safe(&group.name)),
        json!({"ok": true}),
    ))
}

pub(super) fn assign(ctx: &CliContext, group: &str, tag: &str) -> Result<Output, CliError> {
    let group = resolve_group(ctx, group)?;
    ctx.app.assign_tag(group.id, tag)?;
    Ok(Output::new(
        format!("assigned {tag} to {}\n", display_safe(&group.name)),
        json!({"ok": true}),
    ))
}

pub(super) fn move_tag(
    ctx: &CliContext,
    from: &str,
    to: &str,
    tag: &str,
) -> Result<Output, CliError> {
    let from = resolve_group(ctx, from)?;
    let to = resolve_group(ctx, to)?;
    ctx.app.move_tag(from.id, to.id, tag)?;
    Ok(Output::new(
        format!(
            "moved {tag} from {} to {}\n",
            display_safe(&from.name),
            display_safe(&to.name)
        ),
        json!({"ok": true}),
    ))
}

pub(super) fn color_set(ctx: &CliContext, tag: &str, token: &str) -> Result<Output, CliError> {
    let color = parse_color_token(token)?;
    let tag = ctx.app.set_tag_color(tag, color, None)?;
    Ok(Output::new(
        format!(
            "tag {} : {}\n",
            display_safe(&tag.name),
            tag.color.as_token()
        ),
        json!({"tag": to_json(&tag)?}),
    ))
}
