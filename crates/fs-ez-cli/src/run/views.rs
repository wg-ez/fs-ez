//! Commandes des vues sauvegardées : création, liste, suppression et exécution.

use fs_ez_app::ViewInput;
use fs_ez_core::{FilterExpr, SavedView, display_safe};
use serde_json::{Value, json};

use crate::context::CliContext;
use crate::error::CliError;
use crate::output::Output;
use crate::render;

use super::common::{
    ensure_print0_allowed, files_page_output, parse_color_token, resolve_view, resolve_view_group,
    to_json,
};

pub(super) fn list(ctx: &CliContext) -> Result<Output, CliError> {
    let views = ctx.app.list_views()?;
    let payload: Vec<Value> = views.iter().map(view_json).collect::<Result<_, _>>()?;
    Ok(Output::new(
        render::view_table(&views),
        json!({"views": payload}),
    ))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn add(
    ctx: &CliContext,
    name: &str,
    query: Option<&str>,
    filter: Option<&str>,
    icon: Option<&str>,
    color: Option<&str>,
    sort: fs_ez_core::Sort,
    group: Option<&str>,
) -> Result<Output, CliError> {
    let filter = match filter {
        Some(raw) => {
            let filter: FilterExpr =
                serde_json::from_str(raw).map_err(|error| CliError::Json(error.to_string()))?;
            Some(fs_ez_app::coherent_filter(filter, query)?)
        }
        None => match query {
            Some(raw) => Some(fs_ez_app::parse_filter_query(raw)?),
            None => {
                return Err(CliError::Usage(
                    "view add requires --query or --filter".to_string(),
                ));
            }
        },
    };
    let color = color.map(parse_color_token).transpose()?;
    let input = ViewInput {
        name: name.to_string(),
        filter,
        icon: icon.map(str::to_string),
        color,
        position: None,
        sort,
    };
    let view = match group {
        Some(name) => {
            let group = resolve_view_group(ctx, name)?;
            ctx.app.create_view_in_group(input, Some(group.id))?
        }
        None => ctx.app.create_view(input)?,
    };
    Ok(Output::new(
        format!("created view {}\n", display_safe(&view.name)),
        json!({"view": view_json(&view)?}),
    ))
}

fn view_json(view: &SavedView) -> Result<Value, CliError> {
    let mut value = to_json(view)?;
    value["query"] = json!(fs_ez_query::pretty(&view.filter));
    Ok(value)
}

pub(super) fn remove(ctx: &CliContext, name: &str) -> Result<Output, CliError> {
    let view = resolve_view(ctx, name)?;
    ctx.app.delete_view(view.id)?;
    Ok(Output::new(
        format!("deleted view {}\n", display_safe(&view.name)),
        json!({"ok": true}),
    ))
}

pub(super) fn run(
    ctx: &CliContext,
    name: &str,
    limit: usize,
    offset: usize,
    print0: bool,
) -> Result<Output, CliError> {
    ensure_print0_allowed(ctx.globals.json, print0)?;
    let view = resolve_view(ctx, name)?;
    let page = ctx.app.run_view(view.id, offset, limit)?;
    files_page_output(page, print0)
}
