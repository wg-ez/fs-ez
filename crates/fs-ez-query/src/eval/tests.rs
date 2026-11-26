use super::*;
use crate::parse;
use std::collections::BTreeSet;

fn tags(names: &[&str]) -> Vec<EvalTag> {
    names
        .iter()
        .map(|tag| EvalTag::new(tag).expect("tag de test valide"))
        .collect()
}

fn eval(query: &str, names: &[&str]) -> bool {
    let expr = parse(query).expect("requête de test valide");
    evaluate(&expr, &tags(names))
}

fn tag(name: &str) -> FilterExpr {
    FilterExpr::Tag {
        tag: name.to_string(),
        negated: false,
    }
}

#[test]
fn new_normalizes_tag() {
    let applied = EvalTag::new("  Été  ").expect("tag valide");
    assert_eq!(applied.tag_norm, "été");
}

#[test]
fn new_rejects_invalid_names() {
    assert_eq!(
        EvalTag::new("a,b"),
        Err(QueryError::InvalidName("a,b".to_string()))
    );
}

#[test]
fn reference_examples() {
    assert!(eval("facture and not paye", &["facture", "2026"]));
    assert!(!eval("facture and not paye", &["facture", "paye"]));
    assert!(!eval("not (a or b)", &["a"]));
}

#[test]
fn matching_is_case_insensitive_after_normalization() {
    assert!(eval("ÉTÉ", &["été"]));
}

#[test]
fn composite_nodes_follow_boolean_tables() {
    let only_a = ["a"];
    assert!(eval("(a and b) or a", &only_a));
    assert!(!eval("(a and b) or b", &only_a));
    assert!(eval("not (a or b)", &["c"]));
    assert!(!evaluate(&FilterExpr::And { items: vec![] }, &[]));
    assert!(!evaluate(&FilterExpr::Or { items: vec![] }, &[]));
    assert!(evaluate(
        &FilterExpr::Not {
            item: Box::new(FilterExpr::And { items: vec![] }),
        },
        &[]
    ));
}

#[test]
fn invalid_names_make_the_tag_node_false() {
    let empty: [EvalTag; 0] = [];
    assert!(!evaluate(&tag("a,b"), &empty));
    assert!(!evaluate(
        &FilterExpr::Tag {
            tag: "a,b".to_string(),
            negated: true,
        },
        &empty
    ));
}

#[test]
fn all_tag_subsets_match_naive_reference() {
    let universe = ["a", "b", "c"];
    let filters = [
        parse("a and not b").expect("requête de test valide"),
        parse("a or b").expect("requête de test valide"),
        parse("not (a or b)").expect("requête de test valide"),
    ];
    let mut checked = 0;
    for mask in 0u8..(1 << universe.len()) {
        let subset: Vec<EvalTag> = universe
            .iter()
            .enumerate()
            .filter(|(index, _)| mask & (1 << index) != 0)
            .map(|(_, tag)| EvalTag::new(tag).expect("tag de test valide"))
            .collect();
        for expr in &filters {
            assert_eq!(evaluate(expr, &subset), naive(expr, &subset), "mask={mask}");
            checked += 1;
        }
    }
    assert_eq!(checked, 24);
}

fn naive(expr: &FilterExpr, tags: &[EvalTag]) -> bool {
    match expr {
        FilterExpr::Tag { tag, negated } => {
            let name = tag.trim().to_lowercase();
            let result = tags.iter().any(|applied| applied.tag_norm == name);
            if *negated { !result } else { result }
        }
        FilterExpr::OnlyTag { tag, negated } => {
            let target = EvalTag::new(tag).expect("tag valide").tag_norm;
            let result = !tags.is_empty() && tags.iter().all(|applied| applied.tag_norm == target);
            if *negated { !result } else { result }
        }
        FilterExpr::TagSet {
            tags: wanted,
            negated,
        } => {
            let expected: BTreeSet<String> = wanted
                .iter()
                .map(|tag| EvalTag::new(tag).expect("tag valide").tag_norm)
                .collect();
            let actual: BTreeSet<&str> = tags
                .iter()
                .map(|applied| applied.tag_norm.as_str())
                .collect();
            let result =
                actual.iter().all(|tag| expected.contains(*tag)) && actual.len() == expected.len();
            if *negated { !result } else { result }
        }
        FilterExpr::And { items } => {
            !items.is_empty() && items.iter().all(|item| naive(item, tags))
        }
        FilterExpr::Or { items } => items.iter().any(|item| naive(item, tags)),
        FilterExpr::Not { item } => !naive(item, tags),
    }
}
