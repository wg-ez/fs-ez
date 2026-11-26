//! Helpers partagés par les pages du shell : erreur, préférences et base.

use askama::Template;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use fs_ez_core::Settings;

use crate::backend::{Backend, blocking};
use crate::ctx::{Base, SidebarData};
use crate::error::WebError;
use crate::handlers::ui::chrome::{self, UiQuery};
use crate::handlers::ui::section;
use crate::handlers::ui::sidebar::build_sidebar;
use crate::htmx;
pub(crate) use crate::prefs::Prefs;
use crate::state::AppState;
use crate::views::{ErrorTemplate, SidebarOobTemplate};

pub(crate) fn render_error<B: Backend>(
    state: &AppState<B>,
    lang: &str,
    theme: &str,
    username: &str,
) -> Response {
    let base = chrome::base(
        state.locales.clone(),
        lang,
        theme,
        username.to_string(),
        state.backend.version(),
        String::new(),
    );
    let message = base.t("error.internal");
    let template = ErrorTemplate { base, message };
    match template.render() {
        Ok(html) => (StatusCode::INTERNAL_SERVER_ERROR, Html(html)).into_response(),
        Err(error) => {
            tracing::error!(%error, "error template failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
        }
    }
}

/// Rendu d'un template Askama, échec transformé en 500.
pub(crate) fn render<T: Template>(template: T) -> Response {
    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(error) => render_failure(error),
    }
}

/// Échec de rendu Askama journalisé, réponse 500 générique.
pub(crate) fn render_failure(error: askama::Error) -> Response {
    tracing::error!(%error, "template failed");
    (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
}

/// Action de la partie spécifique après chargement de la sidebar.
pub(crate) enum FinishAction {
    /// Continue avec le déclencheur fourni.
    Continue,
    /// Continue sans déclencheur HTMX.
    WithoutTrigger,
    /// Court-circuite le rendu par une réponse immédiate.
    Respond(Response),
}

/// Fin de mutation : sidebar OOB rechargée et déclencheur HTMX.
pub(crate) async fn finish_sidebar<B, F>(
    state: &AppState<B>,
    ui: &UiQuery,
    headers: &HeaderMap,
    trigger: Option<&'static str>,
    customize: F,
) -> Response
where
    B: Backend,
    F: FnOnce(&mut SidebarData, &Settings, &str) -> FinishAction,
{
    let (settings, mut sidebar) = match blocking(state.backend.app(), build_sidebar).await {
        Ok(snapshot) => snapshot,
        Err(error) => return error.into_response(),
    };
    let lang = chrome::resolve_lang(headers, ui, Some(&settings));
    let trigger = match customize(&mut sidebar, &settings, &lang) {
        FinishAction::Continue => trigger,
        FinishAction::WithoutTrigger => None,
        FinishAction::Respond(response) => return response,
    };
    let base = page_base(
        state,
        &lang,
        settings.theme.as_token(),
        "",
        &section::from_headers(headers),
        &settings,
    );
    render_sidebar(base, sidebar, trigger)
}

/// Sidebar hors bande, déclencheur HTMX optionnel.
pub(crate) fn render_sidebar(
    base: Base,
    sidebar: SidebarData,
    trigger: Option<&'static str>,
) -> Response {
    let template = SidebarOobTemplate { base, sidebar };
    match template.render() {
        Ok(html) => {
            let response = Html(html).into_response();
            match trigger {
                Some(trigger) => htmx::with_trigger(response, trigger),
                None => response,
            }
        }
        Err(error) => render_failure(error),
    }
}

/// Retour au référent ou racine, statut 303.
pub(crate) fn redirect_back(headers: &HeaderMap) -> Response {
    let target = crate::redirect::referer_target(headers, "/");
    (StatusCode::SEE_OTHER, [(header::LOCATION, target)]).into_response()
}

/// Réglages persistés, l'erreur est propagée telle quelle.
pub(crate) async fn load_settings<B: Backend>(state: &AppState<B>) -> Result<Settings, WebError> {
    blocking(state.backend.app(), |app| app.settings()).await
}

/// Langue et thème effectifs, réglages persistés ou absents.
pub(crate) fn prefs_opt(settings: Option<&Settings>, headers: &HeaderMap, ui: &UiQuery) -> Prefs {
    chrome::resolve(headers, ui, settings)
}

/// Langue et thème effectifs d'après les réglages persistés.
pub(crate) fn prefs(settings: &Settings, headers: &HeaderMap, ui: &UiQuery) -> Prefs {
    prefs_opt(Some(settings), headers, ui)
}

/// Langue et thème de secours avant chargement des réglages.
pub(crate) fn fallback_prefs(headers: &HeaderMap, ui: &UiQuery) -> Prefs {
    prefs_opt(None, headers, ui)
}

/// Langue effective d'après les réglages persistés, sans exposer ceux-ci.
pub(crate) async fn load_lang<B: Backend>(
    state: &AppState<B>,
    headers: &HeaderMap,
    ui: &UiQuery,
) -> Result<String, WebError> {
    Ok(load_settings_and_lang(state, headers, ui).await?.1)
}

/// Réglages persistés et langue effective en un seul chargement.
pub(crate) async fn load_settings_and_lang<B: Backend>(
    state: &AppState<B>,
    headers: &HeaderMap,
    ui: &UiQuery,
) -> Result<(Settings, String), WebError> {
    let settings = load_settings(state).await?;
    let lang = chrome::resolve_lang(headers, ui, Some(&settings));
    Ok((settings, lang))
}

/// Base du shell, préférences et sidebar repliée selon l'état persisté.
pub(crate) fn page_base<B: Backend>(
    state: &AppState<B>,
    lang: &str,
    theme: &str,
    username: &str,
    active: &str,
    settings: &Settings,
) -> Base {
    let mut base = chrome::base(
        state.locales.clone(),
        lang,
        theme,
        username.to_string(),
        state.backend.version(),
        active.to_string(),
    );
    base.collapsed = settings.sidebar_collapsed;
    base.set_settings(settings);
    base
}
