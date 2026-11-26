//! Handlers d'authentification : page `/login`, session JSON et logout.

use std::net::SocketAddr;
use std::time::Instant;

use askama::Template;
use axum::Json;
use axum::extract::{ConnectInfo, Extension, Form, Query, State};
use axum::http::header::{ACCEPT_LANGUAGE, RETRY_AFTER, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use fs_ez_core::Language;
use serde::Deserialize;
use serde_json::json;

use crate::api::errors::SCHEMA_VERSION;
use crate::auth::guards::{SessionUser, no_user_redirect};
use crate::auth::login::WINDOW_SECONDS;
use crate::auth::session::{clear_cookie, login_response, read_cookie};
use crate::backend::{Backend, blocking};
use crate::error::WebError;
use crate::locales::Locales;
use crate::state::AppState;

pub(crate) const MAX_USERNAME_CHARS: usize = 64;
pub(crate) const MAX_PASSWORD_BYTES: usize = 1024;

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub next: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct LoginQuery {
    #[serde(default)]
    pub next: Option<String>,
}

#[derive(Debug, Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    pub lang: String,
    pub title: String,
    pub username_label: String,
    pub password_label: String,
    pub submit_label: String,
    pub error: Option<String>,
    pub next: Option<String>,
    pub username: String,
    pub asset_version: &'static str,
}

pub async fn login_page<B: Backend>(
    State(state): State<AppState<B>>,
    headers: HeaderMap,
    Query(query): Query<LoginQuery>,
) -> Response {
    if let Some(response) = no_user_redirect(&state).await {
        return response;
    }
    if let Some(token) = read_cookie(&headers)
        && let Ok(Some(_)) =
            blocking(state.backend.app(), move |app| app.validate_session(&token)).await
    {
        return Redirect::to("/").into_response();
    }
    let lang = resolve_lang(&headers, None, &state.locales);
    render_login(&state, &lang, query.next, None, "", StatusCode::OK)
}

pub async fn login_submit<B: Backend>(
    State(state): State<AppState<B>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Form(form): Form<LoginForm>,
) -> Response {
    let ip = addr.ip();
    let now = Instant::now();
    // Réservation atomique avant Argon2 : un burst concurrent reste borné par la fenêtre.
    let allowed = state.login_limiter.lock().await.check_and_record(ip, now);
    let lang = resolve_lang(&headers, None, &state.locales);
    let next = safe_next(form.next.as_deref());
    let login_error = || state.locales.t(&lang, "login.error");
    if !allowed {
        let mut response = render_login(
            &state,
            &lang,
            Some(next),
            Some(login_error()),
            "",
            StatusCode::TOO_MANY_REQUESTS,
        );
        response
            .headers_mut()
            .insert(RETRY_AFTER, HeaderValue::from(WINDOW_SECONDS));
        return response;
    }

    let username = form.username.trim().to_string();
    if username.chars().count() > MAX_USERNAME_CHARS || form.password.len() > MAX_PASSWORD_BYTES {
        return render_login(
            &state,
            &lang,
            Some(next),
            Some(login_error()),
            &username,
            StatusCode::OK,
        );
    }

    let password = form.password;
    let login_username = username.clone();
    let login = blocking(state.backend.app(), move |app| {
        app.login(&login_username, &password)
    })
    .await;

    match login {
        Ok(session) => {
            state.login_limiter.lock().await.clear(ip);
            login_response(&headers, &session.token, &next)
        }
        Err(WebError::BadRequest(_)) => render_login(
            &state,
            &lang,
            Some(next),
            Some(login_error()),
            &username,
            StatusCode::OK,
        ),
        Err(error) => error.into_response(),
    }
}

pub async fn logout<B: Backend>(State(state): State<AppState<B>>, headers: HeaderMap) -> Response {
    if let Some(token) = read_cookie(&headers)
        && let Err(error) = blocking(state.backend.app(), move |app| app.logout(&token)).await
    {
        tracing::warn!(%error, "session logout failed");
    }
    let cookie = clear_cookie(crate::cookie_util::secure_from_headers(&headers));
    ([(SET_COOKIE, cookie.to_string())], Redirect::to("/login")).into_response()
}

pub async fn api_session(Extension(user): Extension<SessionUser>) -> Json<serde_json::Value> {
    Json(json!({ "schema": SCHEMA_VERSION, "username": user.username }))
}

fn render_login<B: Backend>(
    state: &AppState<B>,
    lang: &str,
    next: Option<String>,
    error: Option<String>,
    username: &str,
    status: StatusCode,
) -> Response {
    let template = LoginTemplate {
        lang: lang.to_string(),
        title: state.locales.t(lang, "login.title"),
        username_label: state.locales.t(lang, "login.username"),
        password_label: state.locales.t(lang, "login.password"),
        submit_label: state.locales.t(lang, "login.submit"),
        error,
        next,
        username: username.to_string(),
        asset_version: crate::assets::ASSET_VERSION,
    };
    match template.render() {
        Ok(html) => (status, Html(html)).into_response(),
        Err(error) => WebError::Internal(format!("login template: {error}")).into_response(),
    }
}

/// Seules les cibles locales d'un seul `/` sont acceptées, jamais `//` ni une URL absolue.
fn safe_next(raw: Option<&str>) -> String {
    raw.and_then(crate::redirect::local_target)
        .unwrap_or_else(|| "/".to_string())
}

/// `?lang=` valide sinon langue de l'en-tête : même règle pour login et setup.
pub(crate) fn resolve_lang(
    headers: &HeaderMap,
    requested: Option<&str>,
    locales: &Locales,
) -> String {
    match requested {
        Some(lang) if locales.has_lang(lang) => lang.to_string(),
        _ => preferred_lang(headers),
    }
}

pub(crate) fn preferred_lang(headers: &HeaderMap) -> String {
    let requested = headers
        .get_all(ACCEPT_LANGUAGE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .collect::<String>();
    crate::prefs::preferred(&requested)
        .unwrap_or(Language::En)
        .as_token()
        .to_string()
}

#[cfg(test)]
mod tests;
