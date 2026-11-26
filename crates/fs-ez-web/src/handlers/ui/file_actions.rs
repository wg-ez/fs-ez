//! Actions de fichier du menu contextuel : renommage et suppression.

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use fs_ez_core::FileId;

use crate::backend::{Backend, blocking};
use crate::handlers::ui::chrome::UiQuery;
use crate::handlers::ui::form::{UiForm, first_value};
use crate::handlers::ui::pages::load_lang;
use crate::handlers::ui::selection::parse_selection;
use crate::htmx::page_changed;
use crate::state::AppState;

pub async fn rename_file<B: Backend>(
    State(state): State<AppState<B>>,
    Path(id): Path<FileId>,
    Query(ui): Query<UiQuery>,
    headers: HeaderMap,
    UiForm(pairs): UiForm,
) -> Response {
    let lang = match load_lang(&state, &headers, &ui).await {
        Ok(lang) => lang,
        Err(error) => return error.into_response(),
    };
    let Some(name) = first_value(&pairs, "name") else {
        return page_changed(
            &headers,
            false,
            state.locales.t(&lang, "files.rename_failed"),
        );
    };
    let name = name.to_string();
    let result = blocking(state.backend.app(), move |app| app.rename_file(id, &name)).await;
    match result {
        Ok(_) => {
            let message = state.locales.t(&lang, "files.rename_done");
            page_changed(&headers, true, message)
        }
        Err(error) => {
            let message = format!("{}: {error}", state.locales.t(&lang, "files.rename_failed"));
            page_changed(&headers, false, message)
        }
    }
}

pub async fn delete_files<B: Backend>(
    State(state): State<AppState<B>>,
    Query(ui): Query<UiQuery>,
    headers: HeaderMap,
    UiForm(pairs): UiForm,
) -> Response {
    let lang = match load_lang(&state, &headers, &ui).await {
        Ok(lang) => lang,
        Err(error) => return error.into_response(),
    };
    let selection = match parse_selection(&pairs) {
        Ok(selection) => selection,
        Err(error) => return page_changed(&headers, false, error.to_string()),
    };
    let result = blocking(state.backend.app(), move |app| {
        let ids = selection.resolve(app)?;
        if ids.is_empty() {
            return Ok(None);
        }
        app.delete_files(&ids).map(Some)
    })
    .await;
    let report = match result {
        Ok(Some(report)) => report,
        Ok(None) => {
            return page_changed(&headers, false, state.locales.t(&lang, "files.bulk.none"));
        }
        Err(error) => {
            return page_changed(
                &headers,
                false,
                format!("{}: {error}", state.locales.t(&lang, "files.bulk.failed")),
            );
        }
    };
    let mut message = format!(
        "{} {}",
        report.deleted,
        state.locales.t(&lang, "files.bulk.deleted")
    );
    if report.failed > 0 {
        message.push_str(&format!(
            " · {} {}",
            report.failed,
            state.locales.t(&lang, "files.bulk.failed")
        ));
    }
    page_changed(&headers, report.deleted > 0 || report.failed == 0, message)
}
