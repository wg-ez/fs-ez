//! Page réglages : préférences de navigation, inbox et gestion des clés d'API.

use axum::extract::{Extension, Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Redirect, Response};
use fs_ez_core::ApiTokenId;
use fs_ez_store::{ApiTokenRecord, STALE_CREDENTIAL_GENERATION, normalize_api_token_name};
use time::Date;
use time::OffsetDateTime;
use time::macros::format_description;

use crate::api::auth::AuthSource;
use crate::auth::SessionUser;
use crate::backend::{Backend, blocking};
use crate::error::WebError;
use crate::handlers::ui::chrome::UiQuery;
use crate::handlers::ui::form::{UiForm, first_value};
use crate::handlers::ui::pages::{Prefs, load_lang, load_settings, page_base, prefs, render};
use crate::handlers::ui::sidebar::build_sidebar;
use crate::htmx;
use crate::state::AppState;
use crate::views::settings::TokensData;
use crate::views::{SettingsPageTemplate, SettingsTokensTemplate};

pub async fn settings_page<B: Backend>(
    State(state): State<AppState<B>>,
    Extension(source): Extension<AuthSource>,
    Extension(user): Extension<SessionUser>,
    Query(ui): Query<UiQuery>,
    headers: HeaderMap,
) -> Response {
    if let Err(error) = require_session(source) {
        return error.into_response();
    }
    let records = match list_tokens(&state).await {
        Ok(records) => records,
        Err(error) => return error.into_response(),
    };
    render_settings_page(
        &state,
        &user,
        &ui,
        &headers,
        TokensData::new(records, None, None, String::new(), String::new()),
    )
    .await
}

pub async fn create_token<B: Backend>(
    State(state): State<AppState<B>>,
    Extension(source): Extension<AuthSource>,
    Extension(user): Extension<SessionUser>,
    Query(ui): Query<UiQuery>,
    headers: HeaderMap,
    UiForm(pairs): UiForm,
) -> Response {
    if let Err(error) = require_session(source) {
        return error.into_response();
    }
    let lang = match load_lang(&state, &headers, &ui).await {
        Ok(lang) => lang,
        Err(error) => return error.into_response(),
    };
    let draft_name = first_value(&pairs, "name").unwrap_or_default().to_string();
    let draft_expires = first_value(&pairs, "expires_at")
        .unwrap_or_default()
        .to_string();
    let Some(name) = normalize_api_token_name(&draft_name) else {
        return render_creation_error(
            &state,
            &user,
            &ui,
            &headers,
            &lang,
            "tokens.name_invalid",
            TokenDraft {
                name: draft_name,
                expires: draft_expires,
            },
        )
        .await;
    };
    let expires_at = match parse_expiration(draft_expires.trim()) {
        Ok(expires_at) => expires_at,
        Err(()) => {
            return render_creation_error(
                &state,
                &user,
                &ui,
                &headers,
                &lang,
                "tokens.expires_invalid",
                TokenDraft {
                    name: draft_name,
                    expires: draft_expires,
                },
            )
            .await;
        }
    };
    let outcome = blocking(state.backend.app(), move |app| {
        app.create_api_token_for_generation(&name, expires_at, user.credential_generation)
    })
    .await;
    let (created, error_key) = match outcome {
        Ok(created) => (Some(created.token), None),
        Err(WebError::Conflict(message)) if message == STALE_CREDENTIAL_GENERATION => {
            return WebError::Conflict(message).into_response();
        }
        Err(WebError::Conflict(_)) => (None, Some("tokens.exists")),
        Err(WebError::BadRequest(_)) => (None, Some("tokens.expires_invalid")),
        Err(error) => return error.into_response(),
    };
    let records = match list_tokens(&state).await {
        Ok(records) => records,
        Err(error) => return error.into_response(),
    };
    let error = error_key.map(|key| state.locales.t(&lang, key));
    render_tokens(
        &state,
        &user,
        &ui,
        &headers,
        TokensData::new(records, created, error, draft_name, draft_expires),
    )
    .await
}

pub async fn revoke_token<B: Backend>(
    State(state): State<AppState<B>>,
    Extension(source): Extension<AuthSource>,
    Extension(user): Extension<SessionUser>,
    Query(ui): Query<UiQuery>,
    Path(id): Path<ApiTokenId>,
    headers: HeaderMap,
) -> Response {
    if let Err(error) = require_session(source) {
        return error.into_response();
    }
    let lang = match load_lang(&state, &headers, &ui).await {
        Ok(lang) => lang,
        Err(error) => return error.into_response(),
    };
    let outcome = blocking(state.backend.app(), move |app| app.revoke_api_token(id)).await;
    let error = match outcome {
        Ok(true) => None,
        Ok(false) => Some(state.locales.t(&lang, "tokens.not_found")),
        Err(error) => return error.into_response(),
    };
    if !htmx::is_htmx(&headers) && error.is_none() {
        return Redirect::to("/settings").into_response();
    }
    let records = match list_tokens(&state).await {
        Ok(records) => records,
        Err(error) => return error.into_response(),
    };
    render_tokens(
        &state,
        &user,
        &ui,
        &headers,
        TokensData::new(records, None, error, String::new(), String::new()),
    )
    .await
}

async fn render_creation_error<B: Backend>(
    state: &AppState<B>,
    user: &SessionUser,
    ui: &UiQuery,
    headers: &HeaderMap,
    lang: &str,
    key: &str,
    draft: TokenDraft,
) -> Response {
    let records = match list_tokens(state).await {
        Ok(records) => records,
        Err(error) => return error.into_response(),
    };
    render_tokens(
        state,
        user,
        ui,
        headers,
        TokensData::new(
            records,
            None,
            Some(state.locales.t(lang, key)),
            draft.name,
            draft.expires,
        ),
    )
    .await
}

struct TokenDraft {
    name: String,
    expires: String,
}

async fn render_tokens<B: Backend>(
    state: &AppState<B>,
    user: &SessionUser,
    ui: &UiQuery,
    headers: &HeaderMap,
    data: TokensData,
) -> Response {
    if htmx::is_htmx(headers) {
        let settings = match load_settings(state).await {
            Ok(settings) => settings,
            Err(error) => return error.into_response(),
        };
        let Prefs { lang, theme } = prefs(&settings, headers, ui);
        let base = page_base(state, &lang, &theme, &user.username, "settings", &settings);
        return htmx::with_vary(render(SettingsTokensTemplate { base, data }));
    }
    render_settings_page(state, user, ui, headers, data).await
}

/// Page complète des réglages : sidebar chargée, préférences résolues.
async fn render_settings_page<B: Backend>(
    state: &AppState<B>,
    user: &SessionUser,
    ui: &UiQuery,
    headers: &HeaderMap,
    data: TokensData,
) -> Response {
    let snapshot = match blocking(state.backend.app(), build_sidebar).await {
        Ok(snapshot) => snapshot,
        Err(error) => return error.into_response(),
    };
    let (settings, sidebar) = snapshot;
    let Prefs { lang, theme } = prefs(&settings, headers, ui);
    let base = page_base(state, &lang, &theme, &user.username, "settings", &settings);
    render(SettingsPageTemplate {
        base,
        sidebar,
        data,
        q: String::new(),
        query: String::new(),
    })
}

async fn list_tokens<B: Backend>(state: &AppState<B>) -> Result<Vec<ApiTokenRecord>, WebError> {
    blocking(state.backend.app(), |app| app.list_api_tokens()).await
}

/// Un jeton compromis ne doit pouvoir ni lister, ni créer, ni révoquer les autres.
pub(crate) fn require_session(source: AuthSource) -> Result<(), WebError> {
    match source {
        AuthSource::Session => Ok(()),
        AuthSource::Bearer => Err(WebError::Forbidden),
    }
}

/// `jj-mm-aaaa` vide = pas d'expiration, une date passée est refusée par `App`.
fn parse_expiration(raw: &str) -> Result<Option<OffsetDateTime>, ()> {
    if raw.is_empty() {
        return Ok(None);
    }
    let format = format_description!("[day]-[month]-[year]");
    Date::parse(raw, &format)
        .map(|date| Some(date.midnight().assume_utc()))
        .map_err(|_| ())
}
