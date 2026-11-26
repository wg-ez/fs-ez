//! Réglages de sauvegarde et déclenchement manuel.

use axum::extract::{Extension, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};

use crate::api::auth::AuthSource;
use crate::auth::SessionUser;
use crate::backend::{Backend, blocking};
use crate::error::WebError;
use crate::handlers::ui::chrome::UiQuery;
use crate::handlers::ui::form::{UiForm, first_value};
use crate::handlers::ui::pages::{Prefs, load_settings, prefs, redirect_back, render};
use crate::handlers::ui::preferences::settings_saved;
use crate::handlers::ui::settings::require_session;
use crate::htmx;
use crate::state::AppState;
use crate::views::ToastTemplate;

/// Valide les deux champs avant toute écriture, la fenêtre et l'heure restent cohérentes.
pub async fn update_backup<B: Backend>(
    State(state): State<AppState<B>>,
    Extension(source): Extension<AuthSource>,
    Extension(_user): Extension<SessionUser>,
    Query(ui): Query<UiQuery>,
    headers: HeaderMap,
    UiForm(pairs): UiForm,
) -> Response {
    if let Err(error) = require_session(source) {
        return error.into_response();
    }
    let settings = match load_settings(&state).await {
        Ok(settings) => settings,
        Err(error) => return error.into_response(),
    };
    let mut candidate = settings.clone();
    let days = first_value(&pairs, "backup_days")
        .unwrap_or_default()
        .trim();
    let hour = first_value(&pairs, "backup_hour")
        .unwrap_or_default()
        .trim();
    if let Err(error) = candidate
        .set("io.backup_days", days)
        .and_then(|()| candidate.set("io.backup_hour", hour))
    {
        return WebError::BadRequest(error.to_string()).into_response();
    }
    let outcome = blocking(state.backend.app(), move |app| {
        app.set_setting("io.backup_days", &candidate.backup_days.to_string())?;
        app.set_setting("io.backup_hour", &candidate.backup_hour.to_string())
    })
    .await;
    if let Err(error) = outcome {
        return error.into_response();
    }
    settings_saved(&state, &settings, &headers, &ui)
}

/// Sauvegarde immédiate ; la copie et la purge restent dans `fs-ez-app`.
pub async fn run_backup<B: Backend>(
    State(state): State<AppState<B>>,
    Extension(source): Extension<AuthSource>,
    Extension(_user): Extension<SessionUser>,
    Query(ui): Query<UiQuery>,
    headers: HeaderMap,
) -> Response {
    if let Err(error) = require_session(source) {
        return error.into_response();
    }
    let settings = match load_settings(&state).await {
        Ok(settings) => settings,
        Err(error) => return error.into_response(),
    };
    let result = blocking(state.backend.app(), |app| app.backup()).await;
    let Prefs { lang, .. } = prefs(&settings, &headers, &ui);
    if !htmx::is_htmx(&headers) {
        return redirect_back(&headers);
    }
    match result {
        Ok(_) => render(ToastTemplate {
            message: state.locales.t(&lang, "settings.backup_done"),
            error: false,
        }),
        Err(error) => render(ToastTemplate {
            message: format!(
                "{}: {error}",
                state.locales.t(&lang, "settings.backup_failed")
            ),
            error: true,
        }),
    }
}
