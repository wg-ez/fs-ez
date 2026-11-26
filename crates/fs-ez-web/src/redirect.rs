//! Validation des redirections locales `Location` + `next` de connexion.

use axum::http::{HeaderMap, Uri, header};

/// Cible locale ASCII d'un seul `/`, jamais `//`, une URL absolue, un antislash ni un caractère de contrôle.
pub(crate) fn local_target(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.starts_with('/')
        && !raw.starts_with("//")
        && !raw.contains('\\')
        && !raw.chars().any(char::is_control)
        && raw.is_ascii()
    {
        Some(raw.to_string())
    } else {
        None
    }
}

/// Chemin + query d'une URI de requête, jamais d'hôte ni de schéma.
pub(crate) fn request_target(raw: &str) -> Option<String> {
    let uri: Uri = raw.parse().ok()?;
    let path = uri.path();
    let target = match uri.query() {
        Some(query) => format!("{path}?{query}"),
        None => path.to_string(),
    };
    local_target(&target)
}

/// Chemin local du `Referer`, `fallback` sinon.
pub(crate) fn referer_target(headers: &HeaderMap, fallback: &str) -> String {
    headers
        .get(header::REFERER)
        .and_then(|value| value.to_str().ok())
        .and_then(request_target)
        .unwrap_or_else(|| fallback.to_string())
}

#[cfg(test)]
mod tests;
