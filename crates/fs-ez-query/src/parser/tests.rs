use super::*;

fn tag(name: &str) -> FilterExpr {
    FilterExpr::Tag {
        tag: name.to_string(),
        negated: false,
    }
}

fn negated(name: &str) -> FilterExpr {
    FilterExpr::Tag {
        tag: name.to_string(),
        negated: true,
    }
}

#[test]
fn and_binds_tighter_than_or() {
    assert_eq!(
        parse("a or b and c").expect("requête valide"),
        FilterExpr::Or {
            items: vec![
                tag("a"),
                FilterExpr::And {
                    items: vec![tag("b"), tag("c")],
                },
            ],
        }
    );
}

#[test]
fn not_single_tag_sets_negated() {
    assert_eq!(parse("not paye").expect("requête valide"), negated("paye"));
    assert_eq!(parse("not not paye").expect("requête valide"), tag("paye"));
}

#[test]
fn only_tag_folds_negation() {
    assert_eq!(
        parse("only paye").expect("requête valide"),
        FilterExpr::OnlyTag {
            tag: "paye".to_string(),
            negated: false,
        }
    );
    assert_eq!(
        parse("not only paye").expect("requête valide"),
        FilterExpr::OnlyTag {
            tag: "paye".to_string(),
            negated: true,
        }
    );
    assert_eq!(
        parse("only").expect("tag nomme only"),
        FilterExpr::Tag {
            tag: "only".to_string(),
            negated: false,
        }
    );
}

#[test]
fn exactly_parses_a_tag_set_and_folds_negation() {
    assert_eq!(
        parse("exactly a").expect("requête valide"),
        FilterExpr::TagSet {
            tags: vec!["a".to_string()],
            negated: false,
        }
    );
    assert_eq!(
        parse("exactly (a and b)").expect("requête valide"),
        FilterExpr::TagSet {
            tags: vec!["a".to_string(), "b".to_string()],
            negated: false,
        }
    );
    assert_eq!(
        parse("not exactly (a and b)").expect("requête valide"),
        FilterExpr::TagSet {
            tags: vec!["a".to_string(), "b".to_string()],
            negated: true,
        }
    );
    assert_eq!(
        parse("exactly").expect("tag nomme exactly"),
        FilterExpr::Tag {
            tag: "exactly".to_string(),
            negated: false,
        }
    );
    assert_eq!(
        parse("exactly ()"),
        Err(QueryError::UnexpectedToken(")".to_string(), 9))
    );
}

#[test]
fn not_group_becomes_not_node() {
    assert_eq!(
        parse("not (a and b)").expect("requête valide"),
        FilterExpr::Not {
            item: Box::new(FilterExpr::And {
                items: vec![tag("a"), tag("b")],
            }),
        }
    );
}

#[test]
fn parentheses_override_precedence() {
    assert_eq!(
        parse("(a or b) and c").expect("requête valide"),
        FilterExpr::And {
            items: vec![
                FilterExpr::Or {
                    items: vec![tag("a"), tag("b")],
                },
                tag("c"),
            ],
        }
    );
}

#[test]
fn chains_are_flattened() {
    assert_eq!(
        parse("a and b and c").expect("requête valide"),
        FilterExpr::And {
            items: vec![tag("a"), tag("b"), tag("c")],
        }
    );
    assert_eq!(
        parse("a or b or c").expect("requête valide"),
        FilterExpr::Or {
            items: vec![tag("a"), tag("b"), tag("c")],
        }
    );
    assert_eq!(
        parse("(a and b) and c").expect("requête valide"),
        FilterExpr::And {
            items: vec![tag("a"), tag("b"), tag("c")],
        }
    );
}

#[test]
fn quoted_names_are_trimmed_to_display_form() {
    assert_eq!(
        parse(r#""  Projet  ""#).expect("requête valide"),
        tag("Projet")
    );
    assert_eq!(
        parse(r#""  Projet client  ""#).expect("requête valide"),
        tag("Projet client")
    );
}

#[test]
fn colon_stays_inside_identifier() {
    assert_eq!(
        parse("projet:client-a").expect("requête valide"),
        tag("projet:client-a")
    );
}

#[test]
fn rejects_empty_query() {
    assert_eq!(parse(""), Err(QueryError::Empty));
    assert_eq!(parse(" \t\n "), Err(QueryError::Empty));
}

#[test]
fn rejects_dangling_operator() {
    assert_eq!(parse("a and"), Err(QueryError::UnexpectedEnd));
    assert_eq!(parse("a or"), Err(QueryError::UnexpectedEnd));
    assert_eq!(parse("not"), Err(QueryError::UnexpectedEnd));
    assert_eq!(
        parse("a="),
        Err(QueryError::UnexpectedToken("=".to_string(), 1))
    );
}

#[test]
fn rejects_empty_parentheses() {
    assert_eq!(
        parse("()"),
        Err(QueryError::UnexpectedToken(")".to_string(), 1))
    );
}

#[test]
fn rejects_isolated_symbols_and_trailing_tokens() {
    assert_eq!(
        parse("a & b"),
        Err(QueryError::UnexpectedToken("&".to_string(), 2))
    );
    assert_eq!(
        parse("a b"),
        Err(QueryError::UnexpectedToken("b".to_string(), 2))
    );
    assert_eq!(
        parse("a=b=c"),
        Err(QueryError::UnexpectedToken("=".to_string(), 1))
    );
    assert_eq!(parse("(a"), Err(QueryError::UnexpectedEnd));
}

#[test]
fn rejects_invalid_names() {
    assert_eq!(
        parse("\"a\tb\""),
        Err(QueryError::InvalidName("a\tb".to_string()))
    );
    assert_eq!(
        parse("a,b"),
        Err(QueryError::InvalidName("a,b".to_string()))
    );
    assert_eq!(
        parse("a|b"),
        Err(QueryError::InvalidName("a|b".to_string()))
    );
}

#[test]
fn rejects_too_many_nodes() {
    let query = (0..300)
        .map(|i| format!("t{i}"))
        .collect::<Vec<_>>()
        .join(" and ");
    assert_eq!(parse(&query), Err(QueryError::TooComplex));
}

#[test]
fn rejects_too_deep_tree() {
    let mut query = "x".to_string();
    for _ in 0..20 {
        query = format!("not (a and {query})");
    }
    assert_eq!(parse(&query), Err(QueryError::TooComplex));
}

#[test]
fn rejects_thousand_parentheses_without_overflow() {
    let query = format!("{}x{}", "(".repeat(1000), ")".repeat(1000));
    assert_eq!(parse(&query), Err(QueryError::TooComplex));
}

#[test]
fn rejects_thousand_negations_without_overflow() {
    let query = format!("{}x", "not ".repeat(1000));
    assert_eq!(parse(&query), Err(QueryError::TooComplex));
}

#[test]
fn parses_at_the_depth_limit() {
    let query = format!(
        "{}x{}",
        "(".repeat(MAX_PARSE_DEPTH),
        ")".repeat(MAX_PARSE_DEPTH)
    );
    assert_eq!(parse(&query).expect("requête valide"), tag("x"));
}
