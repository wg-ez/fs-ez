//! Préférences d'affichage du menu, rescan inbox et bascule de la sidebar.

use std::sync::atomic::Ordering;

use axum::extract::{Extension, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Redirect, Response};
use fs_ez_app::App;
use fs_ez_core::Settings;

use crate::auth::SessionUser;
use crate::backend::{Backend, blocking};
use crate::error::WebError;
use crate::handlers::ui::chrome::UiQuery;
use crate::handlers::ui::form::{UiForm, first_value};
use crate::handlers::ui::pages::{Prefs, load_settings, page_base, prefs, redirect_back, render};
use crate::handlers::ui::section;
use crate::handlers::ui::sidebar::build_sidebar;
use crate::htmx;
use crate::state::{AppState, Event};
use crate::views::{SidebarOobTemplate, ToastTemplate};

/// Enregistre les fenêtres des récents, la taille de page et le tri par défaut.
pub async fn update_preferences<B: Backend>(
    State(state): State<AppState<B>>,
    Extension(_user): Extension<SessionUser>,
    Query(ui): Query<UiQuery>,
    headers: HeaderMap,
    UiForm(pairs): UiForm,
) -> Response {
    let settings = match load_settings(&state).await {
        Ok(settings) => settings,
        Err(error) => return error.into_response(),
    };
    let mut candidate = settings.clone();
    let mut writes: Vec<(String, String)> = Vec::new();

    let recent = first_value(&pairs, "recent_days")
        .unwrap_or_default()
        .trim();
    if let Err(error) = candidate.set("ui.recent_days", recent) {
        return WebError::BadRequest(error.to_string()).into_response();
    }
    writes.push((
        "ui.recent_days".to_string(),
        candidate.recent_days.to_string(),
    ));

    // Champ ajouté après coup : absent d'un ancien formulaire, le réglage reste inchangé.
    if let Some(raw) = first_value(&pairs, "recent_file_days") {
        let raw = raw.trim();
        if let Err(error) = candidate.set("ui.recent_file_days", raw) {
            return WebError::BadRequest(error.to_string()).into_response();
        }
        writes.push((
            "ui.recent_file_days".to_string(),
            candidate.recent_file_days.to_string(),
        ));
    }

    let size = first_value(&pairs, "page_size").unwrap_or_default().trim();
    if let Err(error) = candidate.set("ui.page_size", size) {
        return WebError::BadRequest(error.to_string()).into_response();
    }
    writes.push(("ui.page_size".to_string(), candidate.page_size.to_string()));

    let sort = first_value(&pairs, "sort").unwrap_or_default().trim();
    let Some(parsed) = crate::query_params::parse_sort(sort) else {
        return WebError::BadRequest(format!("unknown sort order {sort:?}")).into_response();
    };
    let field = parsed.field.as_token();
    let dir = parsed.dir.as_token();
    if let Err(error) = candidate
        .set("ui.sort_field", field)
        .and_then(|()| candidate.set("ui.sort_dir", dir))
    {
        return WebError::BadRequest(error.to_string()).into_response();
    }
    writes.push(("ui.sort_field".to_string(), field.to_string()));
    writes.push(("ui.sort_dir".to_string(), dir.to_string()));

    let changed = dedupe(writes, &settings);
    let outcome = blocking(state.backend.app(), move |app| {
        write_settings(app, &changed)
    })
    .await;
    if let Err(error) = outcome {
        return error.into_response();
    }
    settings_saved(&state, &settings, &headers, &ui)
}

/// Confirmation des réglages : toast htmx, sinon retour au référent.
pub(crate) fn settings_saved<B: Backend>(
    state: &AppState<B>,
    settings: &Settings,
    headers: &HeaderMap,
    ui: &UiQuery,
) -> Response {
    if !htmx::is_htmx(headers) {
        return redirect_back(headers);
    }
    let Prefs { lang, .. } = prefs(settings, headers, ui);
    render(ToastTemplate {
        message: state.locales.t(&lang, "settings.saved"),
        error: false,
    })
}

/// Lance un scan inbox en arrière-plan, un seul à la fois, sans bloquer la requête.
pub async fn rescan_inbox<B: Backend>(
    State(state): State<AppState<B>>,
    Extension(_user): Extension<SessionUser>,
    Query(ui): Query<UiQuery>,
    headers: HeaderMap,
) -> Response {
    let settings = match load_settings(&state).await {
        Ok(settings) => settings,
        Err(error) => return error.into_response(),
    };
    let Prefs { lang, .. } = prefs(&settings, &headers, &ui);
    let started = state
        .rescan_running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok();
    if started {
        let app = state.backend.app();
        let running = std::sync::Arc::clone(&state.rescan_running);
        let events = state.events.clone();
        tokio::task::spawn_blocking(move || {
            let result = app.scan_inbox();
            running.store(false, Ordering::SeqCst);
            match result {
                Ok(counts) => {
                    let _ = events.send(Event::DataChanged);
                    let _ = events.send(Event::ImportFinished {
                        imported: counts.imported,
                        duplicates: counts.duplicates,
                    });
                    if counts.skipped > 0 {
                        tracing::warn!(skipped = counts.skipped, "inbox rescan skipped files");
                    }
                }
                Err(error) => tracing::warn!(%error, "inbox rescan failed"),
            }
        });
    }
    if !htmx::is_htmx(&headers) {
        return Redirect::to("/").into_response();
    }
    let key = if started {
        "inbox.rescan_started"
    } else {
        "inbox.rescan_running"
    };
    render(ToastTemplate {
        message: state.locales.t(&lang, key),
        error: false,
    })
}

/// Bascule persistée : OOB pour htmx, redirection sinon.
pub async fn toggle_sidebar<B: Backend>(
    State(state): State<AppState<B>>,
    Extension(user): Extension<SessionUser>,
    Query(ui): Query<UiQuery>,
    headers: HeaderMap,
) -> Response {
    let outcome = blocking(state.backend.app(), |app| {
        let settings = app.settings()?;
        let value = if settings.sidebar_collapsed { "0" } else { "1" };
        app.set_setting("ui.sidebar_collapsed", value)
    })
    .await;
    if let Err(error) = outcome {
        return error.into_response();
    }
    let snapshot = match blocking(state.backend.app(), build_sidebar).await {
        Ok(snapshot) => snapshot,
        Err(error) => return error.into_response(),
    };
    let (settings, sidebar) = snapshot;
    let Prefs { lang, theme } = prefs(&settings, &headers, &ui);
    let base = page_base(
        &state,
        &lang,
        &theme,
        &user.username,
        &section::from_headers(&headers),
        &settings,
    );
    if htmx::is_htmx(&headers) {
        return render(SidebarOobTemplate { base, sidebar });
    }
    redirect_back(&headers)
}

/// Une valeur déjà stockée n'est pas réécrite.
fn dedupe(writes: Vec<(String, String)>, settings: &Settings) -> Vec<(String, String)> {
    let mut kept: Vec<(String, String)> = Vec::new();
    for (key, value) in writes {
        if settings.get(&key).as_deref() == Some(value.as_str()) {
            continue;
        }
        if kept.iter().any(|(candidate, _)| candidate == &key) {
            continue;
        }
        kept.push((key, value));
    }
    kept
}

fn write_settings(app: &App, writes: &[(String, String)]) -> Result<(), fs_ez_app::AppError> {
    for (key, value) in writes {
        app.set_setting(key, value)?;
    }
    Ok(())
}
