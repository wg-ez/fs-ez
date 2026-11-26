//! Normalisation des chemins relatifs.

use std::path::{Path, PathBuf};

/// Chemin relatif sûr : ni absolu, ni `..`, ni `.`, ni composant vide, ni NUL.
pub fn normalize_relative_path(raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.chars().any(char::is_control) {
        return None;
    }
    if Path::new(trimmed).is_absolute() {
        return None;
    }
    let mut out = PathBuf::new();
    // Split manuel : `Path::components` masque les séparateurs doublés et le `.` final.
    for part in trimmed.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return None;
        }
        out.push(part);
    }
    Some(out)
}

#[cfg(test)]
mod tests;
