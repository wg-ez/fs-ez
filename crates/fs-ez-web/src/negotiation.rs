//! Négociation de contenu partagée par les gardes et les handlers.

use axum::http::{HeaderMap, header::ACCEPT};

/// Vrai si un en-tête `Accept` contient `application/json`, casse ignorée.
pub(crate) fn accepts_json(headers: &HeaderMap) -> bool {
    headers
        .get_all(ACCEPT)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .any(|value| value.to_ascii_lowercase().contains("application/json"))
}

#[cfg(test)]
mod tests;
