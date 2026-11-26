//! Normalisation des noms de tags.

use crate::error::CoreError;
use crate::text::has_unsafe_text;

const MAX_NAME_CHARS: usize = 64;

/// Charset interdit : lexer TMSU (fiche 06) + entrées de répertoire WebDAV (`/`, `\`), espace admis pour les tags car les requêtes quotent le nom.
const RESERVED: &[char] = &[
    '\t', ',', '=', '+', '(', ')', '"', '\'', '/', '\\', '<', '>', '!', '&', '|',
];

/// Retourne `(nom_affiché, nom_normalisé)` pour un tag.
pub fn normalize_tag_name(raw: &str) -> Result<(String, String), CoreError> {
    let invalid = || CoreError::InvalidName(raw.to_string());
    let display = raw.trim();
    if display.is_empty() || display.chars().count() > MAX_NAME_CHARS || has_unsafe_text(raw) {
        return Err(invalid());
    }
    if display.chars().any(|c| RESERVED.contains(&c)) {
        return Err(invalid());
    }
    // Pas de NFC/NFD : aucune dépendance Unicode supplémentaire, la casse suffit à l'unicité.
    let normalized = display.to_lowercase();
    Ok((display.to_string(), normalized))
}

#[cfg(test)]
mod tests;
