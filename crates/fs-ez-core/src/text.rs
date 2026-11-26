//! Troncature et assainissement de chaînes.

/// `__proto__` (partout), clés `constructor`/`prototype` exactes, contrôles C0/C1, formats bidi/invisibles et séparateurs U+2028/U+2029.
pub fn has_unsafe_text(raw: &str) -> bool {
    raw.contains("__proto__")
        || matches!(
            raw.trim().to_ascii_lowercase().as_str(),
            "constructor" | "prototype"
        )
        || raw.chars().any(|character| {
            character.is_control()
                || matches!(character, '\u{2028}' | '\u{2029}')
                || is_unsafe_format(character)
        })
}

/// Sortie terminal sûre : contrôles et formats bidi/invisibles remplacés par `?`.
pub fn display_safe(raw: &str) -> String {
    raw.chars()
        .map(|character| {
            if character.is_control() || is_unsafe_format(character) {
                '?'
            } else {
                character
            }
        })
        .collect()
}

/// Plages `Cf` et fillers invisibles à risque d'usurpation visuelle, noms invisibles ou réécriture bidi.
fn is_unsafe_format(character: char) -> bool {
    matches!(
        character,
        '\u{00AD}'
            | '\u{061C}'
            // U+200D (ZWJ) accepté pour les séquences emoji, seuls 200B..200C et 200E..200F sont rejetés.
            | '\u{115F}'..='\u{1160}'
            | '\u{180E}'
            | '\u{200B}'..='\u{200C}'
            | '\u{200E}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206F}'
            | '\u{3164}'
            | '\u{1D173}'..='\u{1D17A}'
            | '\u{E0000}'..='\u{E007F}'
            | '\u{FEFF}'
            | '\u{FFA0}'
            | '\u{FFF9}'..='\u{FFFB}'
    )
}

/// Nom trimé si non vide, ≤ `max_chars` caractères et sans texte à risque, test sur `raw` brut.
pub(crate) fn normalize_label(raw: &str, max_chars: usize) -> Option<String> {
    let label = raw.trim();
    if label.is_empty() || label.chars().count() > max_chars || has_unsafe_text(raw) {
        return None;
    }
    Some(label.to_string())
}

/// Coupe `value` à `max_bytes` octets sans casser un caractère multioctet.
pub fn truncate_utf8(value: &str, max_bytes: usize) -> &str {
    let mut end = max_bytes.min(value.len());
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

#[cfg(test)]
mod tests;

/// Longueur maximale du nom affiché, en octets.
pub const MAX_VIRTUAL_NAME_BYTES: usize = 120;

/// `/`, `\`, NUL et contrôles remplacés, `.` et `..` neutralisés.
pub fn sanitize_component(raw: &str) -> String {
    let mut sanitized = String::with_capacity(raw.len());
    for character in raw.chars() {
        let replaced = matches!(character, '/' | '\\' | '\0') || character.is_control();
        sanitized.push(if replaced { '_' } else { character });
    }
    match sanitized.as_str() {
        "" => "_".to_string(),
        "." => "._".to_string(),
        ".." => "_..".to_string(),
        _ => sanitized,
    }
}

/// Nom virtuel WebDAV : `{nom_sanitizé}`.
pub fn virtual_file_name(file: &crate::file::FileRecord) -> String {
    let base = sanitize_component(&file.name);
    truncate_utf8(&base, MAX_VIRTUAL_NAME_BYTES).to_string()
}

/// Nom unique dans une collection, suffixe `-{id}` puis `-{id}-{n}` si collision.
pub fn unique_child_name(
    used: &mut std::collections::HashSet<std::ffi::OsString>,
    raw: &str,
    id: i64,
) -> std::ffi::OsString {
    let base = sanitize_component(raw);
    let mut candidate = std::ffi::OsString::from(&base);
    if used.contains(&candidate) {
        candidate = std::ffi::OsString::from(format!("{base}-{id}"));
        let mut suffix = 2u64;
        while used.contains(&candidate) {
            candidate = std::ffi::OsString::from(format!("{base}-{id}-{suffix}"));
            suffix = suffix.saturating_add(1);
        }
    }
    used.insert(candidate.clone());
    candidate
}
