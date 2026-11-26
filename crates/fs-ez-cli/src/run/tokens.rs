//! Commandes des jetons d'API.

use fs_ez_core::ApiTokenId;
use fs_ez_core::datetime::{format_rfc3339, parse_rfc3339};
use fs_ez_core::display_safe;
use serde_json::{Value as Json, json};

use crate::context::CliContext;
use crate::error::CliError;
use crate::output::Output;
use crate::render;

pub(super) fn add(ctx: &CliContext, name: &str, expires: Option<&str>) -> Result<Output, CliError> {
    let expires_at = match expires {
        Some(raw) => Some(
            parse_rfc3339(raw)
                .map_err(|_| CliError::Usage(format!("invalid RFC3339 expiration '{raw}'")))?,
        ),
        None => None,
    };
    let created = ctx.app.create_api_token(name, expires_at)?;
    let record = &created.record;
    let human = format!(
        "id: {}\nname: {}\ntoken: {}\nwarning: token affiché une seule fois\n",
        record.id,
        display_safe(&record.name),
        created.token,
    );
    Ok(Output::new(
        human,
        json!({
            "id": record.id,
            "name": record.name,
            "token": created.token,
            "expires_at": record.expires_at.map(format_rfc3339),
        }),
    ))
}

pub(super) fn list(ctx: &CliContext) -> Result<Output, CliError> {
    let tokens = ctx.app.list_api_tokens()?;
    let items: Vec<Json> = tokens
        .iter()
        .map(|token| {
            json!({
                "id": token.id,
                "name": token.name,
                "created_at": format_rfc3339(token.created_at),
                "expires_at": token.expires_at.map(format_rfc3339),
                "last_used_at": token.last_used_at.map(format_rfc3339),
            })
        })
        .collect();
    Ok(Output::new(
        render::token_table(&tokens),
        json!({"tokens": items}),
    ))
}

pub(super) fn revoke(ctx: &CliContext, id: ApiTokenId) -> Result<Output, CliError> {
    if !ctx.app.revoke_api_token(id)? {
        return Err(fs_ez_app::AppError::NotFound(format!("api token {id}")).into());
    }
    Ok(Output::new(
        format!("revoked token {id}\n"),
        json!({"ok": true}),
    ))
}
