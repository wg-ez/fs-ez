//! Commande de réinitialisation de l'utilisateur unique.

use fs_ez_core::display_safe;
use serde_json::json;
use zeroize::Zeroize;

use crate::context::CliContext;
use crate::error::CliError;
use crate::output::Output;

use super::common::read_password;

pub(super) fn reset(ctx: &CliContext, username: Option<&str>) -> Result<Output, CliError> {
    let username = username.unwrap_or("user");
    let mut password = read_password()?;
    let reset = ctx.app.reset_user(username, &password);
    password.zeroize();
    let user = reset?;
    Ok(Output::new(
        format!("user {}\n", display_safe(&user.username)),
        json!({"ok": true, "username": user.username}),
    ))
}
