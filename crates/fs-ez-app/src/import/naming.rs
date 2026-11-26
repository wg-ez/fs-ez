//! Noms de fichiers importés : assainissement, suffixes de collision et temporaires.

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use fs_ez_core::truncate_utf8;
use rand::TryRng;
use rand::rngs::SysRng;

use crate::error::AppError;

const TEMP_EXTENSIONS: [&str; 3] = ["part", "tmp", "crdownload"];
const MAX_FILENAME_BYTES: usize = 200;

/// Longueur du suffixe hexadécimal de `random_suffix` (3 octets).
const SUFFIX_HEX_LEN: usize = 6;

pub(super) fn is_hidden_or_temporary(name: &OsStr) -> bool {
    name.as_bytes().first() == Some(&b'.') || is_temporary(name)
}

/// Dernier composant, séparateurs et contrôles remplacés, pas de point initial, ≤ 200 octets.
pub fn sanitize_filename(original: &str) -> Result<String, AppError> {
    let Some(component) = Path::new(original).file_name().and_then(OsStr::to_str) else {
        return Err(AppError::Invalid("missing file name".to_string()));
    };
    if component.is_empty() || component == "." || component == ".." {
        return Err(AppError::Invalid("invalid file name".to_string()));
    }
    let mut cleaned = String::with_capacity(component.len());
    for ch in component.chars() {
        if ch == '/' || ch == '\\' || ch == '\0' || ch.is_control() {
            cleaned.push('_');
        } else {
            cleaned.push(ch);
        }
    }
    let trimmed = cleaned.trim_start_matches('.');
    if trimmed.is_empty() {
        return Err(AppError::Invalid("empty file name".to_string()));
    }
    let limited = limit_bytes(trimmed, MAX_FILENAME_BYTES);
    if !limited.chars().any(char::is_alphanumeric) {
        return Err(AppError::Invalid(
            "file name has no usable character".to_string(),
        ));
    }
    Ok(limited)
}

fn limit_bytes(name: &str, max: usize) -> String {
    if name.len() <= max {
        return name.to_string();
    }
    let (stem, ext) = split_stem_ext(name);
    if let Some(ext) = ext {
        let suffix = format!(".{ext}");
        if suffix.len() < max {
            let kept = truncate_utf8(stem, max - suffix.len());
            if !kept.is_empty() {
                return format!("{kept}{suffix}");
            }
        }
    }
    truncate_utf8(name, max).to_string()
}

/// Nom scindé en `(stem, extension)`, extension vide traitée comme absente.
fn split_stem_ext(name: &str) -> (&str, Option<&str>) {
    let ext = Path::new(name)
        .extension()
        .and_then(OsStr::to_str)
        .filter(|ext| !ext.is_empty());
    match ext {
        Some(ext) => (&name[..name.len() - ext.len() - 1], Some(ext)),
        None => (name, None),
    }
}

pub(super) fn suffixed_name(name: &str, suffix: &str) -> String {
    match split_stem_ext(name) {
        (stem, Some(ext)) => format!("{stem}-{suffix}.{ext}"),
        (_, None) => format!("{name}-{suffix}"),
    }
}

/// `true` si `candidate` est exactement `suffixed_name(requested, <suffixe aléatoire>)`.
pub(super) fn is_suffixed_copy(requested: &str, candidate: &str) -> bool {
    let (stem, _) = split_stem_ext(candidate);
    stem.match_indices('-').any(|(index, _)| {
        let suffix = &stem[index + 1..];
        suffix.len() == SUFFIX_HEX_LEN
            && suffix
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
            && suffixed_name(requested, suffix) == candidate
    })
}

pub(super) fn random_suffix() -> Result<String, AppError> {
    let mut bytes = [0u8; 3];
    let mut rng = SysRng;
    rng.try_fill_bytes(&mut bytes)
        .map_err(|error| AppError::Invalid(format!("random source failure: {error}")))?;
    Ok(format!("{:02x}{:02x}{:02x}", bytes[0], bytes[1], bytes[2]))
}

pub fn is_temporary(name: &OsStr) -> bool {
    Path::new(name)
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|ext| {
            TEMP_EXTENSIONS
                .iter()
                .any(|candidate| ext.eq_ignore_ascii_case(candidate))
        })
}

#[cfg(test)]
mod tests;
