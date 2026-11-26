//! Évaluation en mémoire d'un `FilterExpr`, référence de sémantique hors SQL.
//! Fuzzing futur hors workspace : `cargo fuzz run eval -- crates/fs-ez-query/fuzz/corpus`, `cargo-fuzz` non ajouté.

use fs_ez_core::{FilterExpr, normalize_tag_name};

use crate::error::QueryError;

/// Tag déjà normalisé appliqué à un fichier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvalTag {
    pub tag_norm: String,
}

impl EvalTag {
    pub fn new(tag: &str) -> Result<Self, QueryError> {
        let (_, tag_norm) = normalize_tag_name(tag).map_err(QueryError::from_core)?;
        Ok(Self { tag_norm })
    }
}

/// Évalue un arbre validé, un nom invalide rend son nœud `Tag` faux sans panique.
pub fn evaluate(expr: &FilterExpr, tags: &[EvalTag]) -> bool {
    match expr {
        FilterExpr::Tag { tag, negated } => eval_tag(tag, *negated, tags),
        FilterExpr::OnlyTag { tag, negated } => eval_only_tag(tag, *negated, tags),
        FilterExpr::TagSet {
            tags: wanted,
            negated,
        } => eval_tag_set(wanted, *negated, tags),
        FilterExpr::And { items } => {
            !items.is_empty() && items.iter().all(|item| evaluate(item, tags))
        }
        FilterExpr::Or { items } => items.iter().any(|item| evaluate(item, tags)),
        FilterExpr::Not { item } => !evaluate(item, tags),
    }
}

fn eval_tag(tag: &str, negated: bool, tags: &[EvalTag]) -> bool {
    let Some(tag_norm) = normalized(tag) else {
        return false;
    };
    let present = tags.iter().any(|applied| applied.tag_norm == tag_norm);
    apply(negated, present)
}

fn eval_only_tag(tag: &str, negated: bool, tags: &[EvalTag]) -> bool {
    let Some(tag_norm) = normalized(tag) else {
        return false;
    };
    let outcome = !tags.is_empty() && tags.iter().all(|applied| applied.tag_norm == tag_norm);
    apply(negated, outcome)
}

fn eval_tag_set(wanted: &[String], negated: bool, tags: &[EvalTag]) -> bool {
    let mut expected = std::collections::BTreeSet::new();
    for tag in wanted {
        let Some(tag_norm) = normalized(tag) else {
            return false;
        };
        expected.insert(tag_norm);
    }
    if expected.is_empty() {
        return false;
    }
    let actual: std::collections::BTreeSet<&str> = tags
        .iter()
        .map(|applied| applied.tag_norm.as_str())
        .collect();
    let outcome =
        actual.iter().all(|tag| expected.contains(*tag)) && actual.len() == expected.len();
    apply(negated, outcome)
}

/// Forme normalisée d'un nom de tag, `None` si invalide.
fn normalized(tag: &str) -> Option<String> {
    normalize_tag_name(tag).ok().map(|(_, tag_norm)| tag_norm)
}

/// Applique la négation au résultat du nœud.
fn apply(negated: bool, outcome: bool) -> bool {
    if negated { !outcome } else { outcome }
}

#[cfg(test)]
mod tests;
