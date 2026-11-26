//! En-têtes et réponses htmx partagés par les handlers.

use axum::http::{HeaderMap, HeaderName, HeaderValue};
use axum::response::Response;

use crate::handlers::ui::pages::{redirect_back, render};
use crate::views::ToastTemplate;

pub(crate) const HX_REQUEST: &str = "hx-request";

pub(crate) fn is_htmx(headers: &HeaderMap) -> bool {
    headers.contains_key(HX_REQUEST)
}

/// `trigger` statique : insertion infaillible, aucune allocation.
pub(crate) fn with_trigger(mut response: Response, trigger: &'static str) -> Response {
    response.headers_mut().insert(
        HeaderName::from_static("hx-trigger"),
        HeaderValue::from_static(trigger),
    );
    response
}

/// `Vary: HX-Request` pour distinguer fragment et page complète dans les caches.
pub(crate) fn with_vary(mut response: Response) -> Response {
    response.headers_mut().insert(
        HeaderName::from_static("vary"),
        HeaderValue::from_static("HX-Request"),
    );
    response
}

/// Toast de mutation, déclencheur `data-changed` en cas de succès, redirection hors htmx.
pub(crate) fn page_changed(headers: &HeaderMap, success: bool, message: String) -> Response {
    if !is_htmx(headers) {
        return redirect_back(headers);
    }
    let response = render(ToastTemplate {
        message,
        error: !success,
    });
    if success {
        with_trigger(response, "data-changed")
    } else {
        response
    }
}

#[cfg(test)]
mod tests;
