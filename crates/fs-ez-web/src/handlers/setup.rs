//! Onboarding : création unique du compte utilisateur tant que la base n'en a aucun.

use askama::Template;
use axum::extract::{Form, Query, State};
use axum::http::header::RETRY_AFTER;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use serde::Deserialize;

use crate::auth::guards::existing_user_redirect;
use crate::auth::session::login_response;
use crate::backend::{Backend, blocking};
use crate::error::WebError;
use crate::handlers::auth::{MAX_PASSWORD_BYTES, MAX_USERNAME_CHARS, resolve_lang};
use crate::state::AppState;

/// Attente conseillée quand un hachage setup est déjà en cours, jamais la fenêtre /login.
const SETUP_BUSY_SECONDS: u64 = 1;

#[derive(Debug, Default, Deserialize)]
pub struct SetupQuery {
    #[serde(default)]
    pub lang: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SetupForm {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub lang: Option<String>,
}

#[derive(Debug, Template)]
#[template(path = "setup.html")]
pub struct SetupTemplate {
    pub lang: String,
    pub title: String,
    pub intro: String,
    pub username_label: String,
    pub password_label: String,
    pub submit_label: String,
    pub error: Option<String>,
    pub username: String,
    pub asset_version: &'static str,
}

pub async fn setup_page<B: Backend>(
    State(state): State<AppState<B>>,
    headers: HeaderMap,
    Query(query): Query<SetupQuery>,
) -> Result<Response, WebError> {
    if let Some(response) = existing_user_redirect(&state).await? {
        return Ok(response);
    }
    let lang = resolve_lang(&headers, query.lang.as_deref(), &state.locales);
    Ok(render(&state, &lang, None, "", StatusCode::OK))
}

pub async fn setup_submit<B: Backend>(
    State(state): State<AppState<B>>,
    headers: HeaderMap,
    Form(form): Form<SetupForm>,
) -> Result<Response, WebError> {
    let lang = resolve_lang(&headers, form.lang.as_deref(), &state.locales);
    // Permis owned : il survit à l'abandon de la future HTTP et couvre tout le hachage.
    let Ok(permit) = state.setup_gate.clone().try_acquire_owned() else {
        return Ok(rate_limited(&state, &lang));
    };
    // Un compte déjà commité gagne sans Argon2, sous le même permis.
    if let Some(response) = existing_user_redirect(&state).await? {
        return Ok(response);
    }
    let username = form.username.trim().to_string();
    if !valid_username(&username) {
        return Ok(render(
            &state,
            &lang,
            Some(state.locales.t(&lang, "setup.username_invalid")),
            &username,
            StatusCode::OK,
        ));
    }
    if form.password.chars().count() < fs_ez_app::MIN_PASSWORD_LEN
        || form.password.len() > MAX_PASSWORD_BYTES
    {
        return Ok(render(
            &state,
            &lang,
            Some(state.locales.t(&lang, "setup.password_invalid")),
            &username,
            StatusCode::OK,
        ));
    }

    let password = form.password;
    let init_username = username.clone();
    let created = blocking(state.backend.app(), move |app| {
        let _permit = permit;
        // Vérification autoritaire dans la même section critique que l'insert.
        if app.has_user()? {
            return Ok(None);
        }
        app.init_user(&init_username, &password)?;
        Ok(Some(app.login(&init_username, &password)?))
    })
    .await;
    let session = match created {
        Ok(Some(session)) => session,
        Ok(None) => return Ok(Redirect::to("/login").into_response()),
        Err(WebError::Conflict(_)) => {
            return Ok(render(
                &state,
                &lang,
                Some(state.locales.t(&lang, "setup.exists")),
                &username,
                StatusCode::CONFLICT,
            ));
        }
        Err(error) => return Err(error),
    };
    Ok(login_response(&headers, &session.token, "/"))
}

fn valid_username(username: &str) -> bool {
    !username.is_empty()
        && username.chars().count() <= MAX_USERNAME_CHARS
        && !username.chars().any(char::is_control)
}

fn rate_limited<B: Backend>(state: &AppState<B>, lang: &str) -> Response {
    let mut response = render(
        state,
        lang,
        Some(state.locales.t(lang, "setup.too_many")),
        "",
        StatusCode::TOO_MANY_REQUESTS,
    );
    response
        .headers_mut()
        .insert(RETRY_AFTER, HeaderValue::from(SETUP_BUSY_SECONDS));
    response
}

fn render<B: Backend>(
    state: &AppState<B>,
    lang: &str,
    error: Option<String>,
    username: &str,
    status: StatusCode,
) -> Response {
    let template = SetupTemplate {
        lang: lang.to_string(),
        title: state.locales.t(lang, "setup.title"),
        intro: state.locales.t(lang, "setup.intro"),
        username_label: state.locales.t(lang, "setup.username"),
        password_label: state.locales.t(lang, "setup.password"),
        submit_label: state.locales.t(lang, "setup.submit"),
        error,
        username: username.to_string(),
        asset_version: crate::assets::ASSET_VERSION,
    };
    match template.render() {
        Ok(html) => (status, Html(html)).into_response(),
        Err(error) => WebError::Internal(format!("setup template: {error}")).into_response(),
    }
}

#[cfg(test)]
mod tests;
