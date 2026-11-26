//! Middlewares transverses : CSRF par `Sec-Fetch-Site` et en-têtes de sécurité.

use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use fs_ez_core::{Language, Theme};

use crate::api::auth::AuthSource;
use crate::backend::{Backend, blocking};
use crate::error::WebError;
use crate::prefs;
use crate::state::AppState;

const CONTENT_SECURITY_POLICY: &str = "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'";
const PERMISSIONS_POLICY: &str = "camera=(), microphone=(), geolocation=()";
const STRICT_TRANSPORT_SECURITY: &str = "max-age=31536000";

/// HSTS émis même en HTTP clair, les navigateurs l'ignorent hors HTTPS.
pub async fn security_headers(req: Request, next: Next) -> Response {
    // DAV n'est pas une UI : CSP frame-ancestors ou XFO cassent les visionneuses PDF navigateur.
    let is_dav = req.uri().path() == "/dav" || req.uri().path().starts_with("/dav/");
    let mut response = next.run(req).await;
    let headers = response.headers_mut();
    headers.insert(
        header::STRICT_TRANSPORT_SECURITY,
        HeaderValue::from_static(STRICT_TRANSPORT_SECURITY),
    );
    if !is_dav {
        headers.insert(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(CONTENT_SECURITY_POLICY),
        );
        headers.insert(
            header::REFERRER_POLICY,
            HeaderValue::from_static("same-origin"),
        );
        headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
        headers.insert(
            "permissions-policy",
            HeaderValue::from_static(PERMISSIONS_POLICY),
        );
    }
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}

pub async fn csrf_guard(req: Request, next: Next) -> Response {
    // Un jeton Bearer n'est pas envoyé automatiquement par un navigateur : pas de CSRF possible.
    if req.extensions().get::<AuthSource>() == Some(&AuthSource::Bearer) {
        return next.run(req).await;
    }
    if !matches!(
        *req.method(),
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    ) {
        return next.run(req).await;
    }
    let Some(site) = req
        .headers()
        .get("sec-fetch-site")
        .and_then(|value| value.to_str().ok())
    else {
        // En-tête absent : client non navigateur, jetons d'API autorisés.
        return next.run(req).await;
    };
    if site.eq_ignore_ascii_case("same-origin") || site.eq_ignore_ascii_case("none") {
        return next.run(req).await;
    }
    WebError::Csrf.into_response()
}

/// Navigation cross-site/same-site : un GET top-level porte le cookie Lax, il ne doit rien muter.
fn is_cross_site_navigation(headers: &HeaderMap) -> bool {
    headers
        .get("sec-fetch-site")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|site| {
            site.eq_ignore_ascii_case("cross-site") || site.eq_ignore_ascii_case("same-site")
        })
}

/// `?lang=`/`?theme=` valides : cookie + réglage en base, jamais d'erreur 400.
pub async fn persist_preferences<B: Backend>(
    State(state): State<AppState<B>>,
    req: Request,
    next: Next,
) -> Response {
    if is_cross_site_navigation(req.headers()) {
        return next.run(req).await;
    }
    let query = prefs::query_map(req.uri().query());
    let secure = crate::cookie_util::secure_from_headers(req.headers());
    let lang = query.get("lang").and_then(|raw| Language::parse(raw));
    let theme = query.get("theme").and_then(|raw| Theme::parse(raw));
    if lang.is_none() && theme.is_none() {
        return next.run(req).await;
    }
    let writes = blocking(state.backend.app(), move |app| {
        if let Some(lang) = lang {
            app.set_setting("ui.language", lang.as_token())?;
        }
        if let Some(theme) = theme {
            app.set_setting("ui.theme", theme.as_token())?;
        }
        Ok(())
    })
    .await;
    if let Err(error) = writes {
        tracing::warn!(%error, "preference persist failed");
    }
    let mut response = next.run(req).await;
    for cookie in [
        lang.map(|lang| prefs::lang_cookie(lang.as_token(), secure)),
        theme.map(|theme| prefs::theme_cookie(theme.as_token(), secure)),
    ]
    .into_iter()
    .flatten()
    {
        if let Ok(value) = HeaderValue::from_str(&cookie.to_string()) {
            response.headers_mut().append(header::SET_COOKIE, value);
        }
    }
    response
}
