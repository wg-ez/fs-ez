//! Normalisation des tags fournis par la CLI et le web.

use std::collections::HashSet;

use fs_ez_core::normalize_tag_name;

use crate::error::AppError;

pub fn parse_tag_name(raw: &str) -> Result<String, AppError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AppError::Invalid("empty tag specification".to_string()));
    }
    let (display, _) = normalize_tag_name(trimmed)?;
    Ok(display)
}

/// Plafond de tags acceptés par requête de mutation.
pub const MAX_TAGS_PER_REQUEST: usize = 256;

pub fn parse_tag_names(raw: &[String]) -> Result<Vec<String>, AppError> {
    if raw.len() > MAX_TAGS_PER_REQUEST {
        return Err(AppError::Invalid(format!(
            "too many tags (max {MAX_TAGS_PER_REQUEST})"
        )));
    }
    let mut inputs = Vec::new();
    let mut seen = HashSet::new();
    for raw in raw {
        let name = parse_tag_name(raw)?;
        if seen.insert(name.to_lowercase()) {
            inputs.push(name);
        }
    }
    Ok(inputs)
}

#[cfg(test)]
mod tests;
