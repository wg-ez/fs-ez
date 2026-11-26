//! Commandes système : initialisation, état et réparation.

use fs_ez_core::display_safe;
use fs_ez_fs::display_lossy;
use serde_json::json;

use crate::context::CliContext;
use crate::error::CliError;
use crate::output::Output;
use crate::render;

pub(super) fn init(ctx: &CliContext) -> Result<Output, CliError> {
    let report = ctx.app.init(&ctx.db_path)?;
    ctx.app.apply_owner_layout()?;
    let human = format!(
        "root: {}\ndb: {}\nfiles: {}\n",
        display_safe(&display_lossy(&report.root)),
        display_safe(&display_lossy(&report.db_path)),
        display_safe(&display_lossy(&report.files)),
    );
    Ok(Output::new(
        human,
        json!({
            "root": display_lossy(&report.root),
            "db": display_lossy(&report.db_path),
            "files": display_lossy(&report.files),
        }),
    ))
}

pub(super) fn status(ctx: &CliContext) -> Result<Output, CliError> {
    let report = ctx.app.status()?;
    let payload = json!({
        "version": report.version,
        "root": display_lossy(&report.root),
        "db_path": display_lossy(&report.db_path),
        "db_size": report.db_size,
        "files": report.files,
        "untagged": report.untagged,
        "recent": report.recent,
        "tags": report.tags,
        "views": report.views,
    });
    Ok(Output::new(render::status_text(&report), payload))
}

pub(super) fn repair(ctx: &CliContext) -> Result<Output, CliError> {
    let report = ctx.app.repair()?;
    Ok(Output::new(
        render::repair_text(&report),
        json!({
            "updated": report.updated,
            "moved": report.moved,
            "missing": report.missing,
        }),
    ))
}
