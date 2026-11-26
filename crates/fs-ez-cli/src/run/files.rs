//! Commande `files` : filtrage, tri, pagination et sortie `--print0`.

use fs_ez_app::{AppError, FileListMode, FileListRequest};
use fs_ez_query::parse as parse_query;

use crate::context::CliContext;
use crate::error::CliError;
use crate::output::Output;

use super::common::{ensure_print0_allowed, files_page_output};

#[allow(clippy::too_many_arguments)]
pub(super) fn files(
    ctx: &CliContext,
    query: Option<&str>,
    sort: fs_ez_core::Sort,
    limit: usize,
    offset: usize,
    untagged: bool,
    recent: bool,
    print0: bool,
) -> Result<Output, CliError> {
    if untagged && recent {
        return Err(CliError::Usage(
            "--untagged cannot be combined with --recent".to_string(),
        ));
    }
    if untagged && query.is_some() {
        return Err(CliError::Usage(
            "a query cannot be combined with --untagged".to_string(),
        ));
    }
    ensure_print0_allowed(ctx.globals.json, print0)?;
    let filter = match query {
        Some(query) => Some(
            parse_query(query)
                .map_err(|error| CliError::App(AppError::Invalid(error.to_string())))?,
        ),
        None => None,
    };
    let mode = if untagged {
        FileListMode::Untagged
    } else if recent {
        FileListMode::Recent
    } else {
        FileListMode::All
    };
    let page = ctx.app.list_files_page(&FileListRequest {
        filter,
        mode,
        sort: Some(sort),
        offset,
        ..FileListRequest::new(limit)
    })?;
    files_page_output(page.into_page(), print0)
}
