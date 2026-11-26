//! Compilation d'un `FilterExpr` en predicat SQL parametre.

use fs_ez_core::{FilterExpr, normalize_tag_name};
use fs_ez_store::{StoreError, StoreResult};
use rusqlite::types::Value;

const MAX_DEPTH: usize = 32;
const MAX_NODES: usize = 256;

const TAG_EXISTS: &str = "EXISTS (SELECT 1 FROM file_tag ft \
     JOIN tag t ON t.id = ft.tag_id \
     WHERE ft.file_id = f.id AND t.name_norm = ?)";

const ONLY_TAG: &str = "(EXISTS (SELECT 1 FROM file_tag ft \
     JOIN tag t ON t.id = ft.tag_id \
     WHERE ft.file_id = f.id AND t.name_norm = ?) \
     AND (SELECT COUNT(DISTINCT ft.tag_id) FROM file_tag ft WHERE ft.file_id = f.id) = 1)";

/// Predicat portant sur la table `file` aliasee `f`, valeurs toujours bindees.
pub fn filter_predicate(expr: &FilterExpr) -> StoreResult<(String, Vec<Value>)> {
    let mut nodes = 0;
    compile(expr, 1, &mut nodes)
}

/// Sous-requete `SELECT f.id FROM file f WHERE <predicat>` pour `IN (...)` et `count_matching`.
pub fn filter_subquery(expr: &FilterExpr) -> StoreResult<(String, Vec<Value>)> {
    let (predicate, values) = filter_predicate(expr)?;
    Ok((format!("SELECT f.id FROM file f WHERE {predicate}"), values))
}

fn compile(
    expr: &FilterExpr,
    depth: usize,
    nodes: &mut usize,
) -> StoreResult<(String, Vec<Value>)> {
    *nodes += 1;
    if depth > MAX_DEPTH || *nodes > MAX_NODES {
        return Err(StoreError::Invalid("filter too complex".to_string()));
    }
    match expr {
        FilterExpr::Tag { tag, negated } => tag_predicate(tag, *negated, TAG_EXISTS),
        FilterExpr::OnlyTag { tag, negated } => tag_predicate(tag, *negated, ONLY_TAG),
        FilterExpr::TagSet { tags, negated } => compile_tag_set(tags, *negated),
        FilterExpr::And { items } => composite(items, " AND ", depth, nodes),
        FilterExpr::Or { items } => composite(items, " OR ", depth, nodes),
        FilterExpr::Not { item } => {
            let (predicate, values) = compile(item, depth + 1, nodes)?;
            Ok((format!("NOT ({predicate})"), values))
        }
    }
}

fn composite(
    items: &[FilterExpr],
    operator: &str,
    depth: usize,
    nodes: &mut usize,
) -> StoreResult<(String, Vec<Value>)> {
    if items.is_empty() {
        return Err(StoreError::Invalid("empty and/or".to_string()));
    }
    let mut predicates = Vec::with_capacity(items.len());
    let mut values = Vec::new();
    for item in items {
        let (predicate, mut item_values) = compile(item, depth + 1, nodes)?;
        predicates.push(predicate);
        values.append(&mut item_values);
    }
    Ok((format!("({})", predicates.join(operator)), values))
}

fn tag_predicate(tag: &str, negated: bool, sql: &str) -> StoreResult<(String, Vec<Value>)> {
    // `evaluate` rend faux un nom invalide, negation comprise : predicat constant.
    let Ok((_, tag_norm)) = normalize_tag_name(tag) else {
        return Ok(impossible());
    };
    Ok(negate(
        sql.to_string(),
        vec![Value::Text(tag_norm)],
        negated,
    ))
}

/// Predicat constant faux, valeurs liees vides.
fn impossible() -> (String, Vec<Value>) {
    ("0".to_string(), Vec::new())
}

fn compile_tag_set(tags: &[String], negated: bool) -> StoreResult<(String, Vec<Value>)> {
    let mut wanted: Vec<String> = Vec::with_capacity(tags.len());
    for tag in tags {
        let Ok((_, tag_norm)) = normalize_tag_name(tag) else {
            return Ok(impossible());
        };
        if !wanted.contains(&tag_norm) {
            wanted.push(tag_norm);
        }
    }
    if wanted.is_empty() {
        return Ok(impossible());
    }
    let placeholders = vec!["?"; wanted.len()].join(", ");
    let sql = format!(
        "((SELECT COUNT(DISTINCT ft.tag_id) FROM file_tag ft WHERE ft.file_id = f.id) = ? \
         AND NOT EXISTS (SELECT 1 FROM file_tag ft \
         JOIN tag t ON t.id = ft.tag_id \
         WHERE ft.file_id = f.id AND t.name_norm NOT IN ({placeholders})))"
    );
    let mut values = Vec::with_capacity(wanted.len() + 1);
    values.push(Value::Integer(wanted.len() as i64));
    values.extend(wanted.into_iter().map(Value::Text));
    Ok(negate(sql, values, negated))
}

fn negate(sql: String, values: Vec<Value>, negated: bool) -> (String, Vec<Value>) {
    if negated {
        (format!("NOT ({sql})"), values)
    } else {
        (sql, values)
    }
}

#[cfg(test)]
mod tests;
