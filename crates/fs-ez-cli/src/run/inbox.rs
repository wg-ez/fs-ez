//! Commande de scan de l'inbox.

use serde_json::json;

use crate::context::CliContext;
use crate::error::CliError;
use crate::output::Output;

pub(super) fn scan(ctx: &CliContext) -> Result<Output, CliError> {
    let counts = ctx.app.scan_inbox()?;
    let human = format!(
        "imported {}\nduplicates {}\nskipped {}\n",
        counts.imported, counts.duplicates, counts.skipped,
    );
    Ok(Output::new(
        human,
        json!({
            "imported": counts.imported,
            "duplicates": counts.duplicates,
            "skipped": counts.skipped,
        }),
    ))
}
