use super::*;

fn tag(name: &str, negated: bool) -> FilterExpr {
    FilterExpr::Tag {
        tag: name.to_string(),
        negated,
    }
}

fn predicate(expr: &FilterExpr) -> String {
    filter_predicate(expr).expect("compilation").0
}

#[test]
fn tag_alone_binds_normalized_name() {
    let (sql, values) = filter_predicate(&tag("  Été  ", false)).expect("compilation");
    assert!(sql.starts_with("EXISTS (SELECT 1 FROM file_tag"));
    assert_eq!(values, vec![Value::Text("été".to_string())]);
}

#[test]
fn negation_wraps_the_base_predicate() {
    let sql = predicate(&tag("t", true));
    assert!(sql.starts_with("NOT (EXISTS ("));
}

#[test]
fn invalid_names_compile_to_false_even_negated() {
    assert_eq!(predicate(&tag("a,b", false)), "0");
    assert_eq!(predicate(&tag("a,b", true)), "0");
}

#[test]
fn only_tag_requires_a_single_distinct_tag() {
    let (sql, values) = filter_predicate(&FilterExpr::OnlyTag {
        tag: "  Été  ".to_string(),
        negated: false,
    })
    .expect("compilation");
    assert!(sql.starts_with("(EXISTS ("));
    assert!(sql.contains("COUNT(DISTINCT ft.tag_id)"));
    assert_eq!(values, vec![Value::Text("été".to_string())]);
    assert!(
        filter_predicate(&FilterExpr::OnlyTag {
            tag: "été".to_string(),
            negated: true,
        })
        .expect("compilation")
        .0
        .starts_with("NOT (")
    );
}

#[test]
fn tag_set_uses_count_and_subset() {
    let (sql, values) = filter_predicate(&FilterExpr::TagSet {
        tags: vec!["Network".to_string(), "Software".to_string()],
        negated: false,
    })
    .expect("compilation");
    assert!(sql.contains("COUNT(DISTINCT ft.tag_id)"));
    assert!(sql.contains("NOT IN (?, ?)"));
    assert_eq!(
        values,
        vec![
            Value::Integer(2),
            Value::Text("network".to_string()),
            Value::Text("software".to_string())
        ]
    );
    assert!(
        filter_predicate(&FilterExpr::TagSet {
            tags: vec!["Network".to_string(), "network".to_string()],
            negated: false,
        })
        .expect("compilation")
        .1
        .iter()
        .filter(|value| matches!(value, Value::Text(_)))
        .count()
            == 1
    );
}

#[test]
fn composites_join_with_parentheses() {
    let expr = FilterExpr::Or {
        items: vec![tag("a", false), tag("b", false)],
    };
    let sql = predicate(&expr);
    assert!(sql.starts_with("(EXISTS ("));
    assert!(sql.contains(" OR EXISTS ("));
}

#[test]
fn empty_and_or_are_rejected() {
    assert!(matches!(
        filter_predicate(&FilterExpr::And { items: vec![] }),
        Err(StoreError::Invalid(_))
    ));
    assert!(matches!(
        filter_predicate(&FilterExpr::Or { items: vec![] }),
        Err(StoreError::Invalid(_))
    ));
}

#[test]
fn complexity_limits_are_enforced() {
    let mut deep = tag("a", false);
    for _ in 0..MAX_DEPTH {
        deep = FilterExpr::Not {
            item: Box::new(deep),
        };
    }
    assert_eq!(
        filter_predicate(&deep)
            .expect_err("trop profond")
            .to_string(),
        "invalid: filter too complex"
    );
    let wide = FilterExpr::And {
        items: (0..MAX_NODES).map(|_| tag("a", false)).collect(),
    };
    assert_eq!(
        filter_predicate(&wide).expect_err("trop large").to_string(),
        "invalid: filter too complex"
    );
}

#[test]
fn subquery_selects_ids_from_file() {
    let (sql, values) = filter_subquery(&tag("facture", false)).expect("compilation");
    assert_eq!(sql, format!("SELECT f.id FROM file f WHERE {TAG_EXISTS}"));
    assert_eq!(values.len(), 1);
}
