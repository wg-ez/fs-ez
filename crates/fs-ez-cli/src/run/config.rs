//! Commandes de consultation et modification des réglages en base.

use serde_json::{Value as Json, json};

use crate::context::CliContext;
use crate::error::CliError;
use crate::output::Output;
use crate::render;

pub(super) fn list(ctx: &CliContext) -> Result<Output, CliError> {
    let settings = ctx.app.all_settings()?;
    let items: Vec<Json> = settings
        .iter()
        .map(|(key, value)| json!({"key": key, "value": value}))
        .collect();
    Ok(Output::new(
        render::config_text(&settings),
        json!({"settings": items}),
    ))
}

pub(super) fn get(ctx: &CliContext, key: &str) -> Result<Output, CliError> {
    let value = ctx.app.get_setting(key)?;
    let human = match &value {
        Some(value) => format!("{key}={value}\n"),
        None => format!("{key}=-\n"),
    };
    Ok(Output::new(human, json!({"key": key, "value": value})))
}

pub(super) fn set(ctx: &CliContext, key: &str, value: &str) -> Result<Output, CliError> {
    ctx.app.set_setting(key, value)?;
    Ok(Output::new(format!("{key}={value}\n"), json!({"ok": true})))
}
