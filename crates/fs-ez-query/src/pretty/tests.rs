use super::*;
use crate::parse;

fn tag(name: &str) -> FilterExpr {
    FilterExpr::Tag {
        tag: name.to_string(),
        negated: false,
    }
}

#[test]
fn prints_simple_terms() {
    assert_eq!(pretty(&tag("a")), "a");
    assert_eq!(
        pretty(&FilterExpr::Tag {
            tag: "a".to_string(),
            negated: true,
        }),
        "not a"
    );
    assert_eq!(
        pretty(&FilterExpr::OnlyTag {
            tag: "a b".to_string(),
            negated: false,
        }),
        "only \"a b\""
    );
    assert_eq!(
        pretty(&FilterExpr::OnlyTag {
            tag: "a".to_string(),
            negated: true,
        }),
        "not only a"
    );
    assert_eq!(
        pretty(&FilterExpr::TagSet {
            tags: vec!["a".to_string(), "b".to_string()],
            negated: false,
        }),
        "exactly (a and b)"
    );
    assert_eq!(
        pretty(&FilterExpr::TagSet {
            tags: vec!["a b".to_string()],
            negated: true,
        }),
        "not exactly \"a b\""
    );
}

#[test]
fn parents_follow_precedence() {
    let expr = FilterExpr::And {
        items: vec![
            FilterExpr::Or {
                items: vec![tag("a"), tag("b")],
            },
            tag("c"),
        ],
    };
    assert_eq!(pretty(&expr), "(a or b) and c");
    let mirrored = FilterExpr::Or {
        items: vec![
            FilterExpr::And {
                items: vec![tag("a"), tag("b")],
            },
            tag("c"),
        ],
    };
    assert_eq!(pretty(&mirrored), "a and b or c");
    assert_eq!(
        pretty(&FilterExpr::Not {
            item: Box::new(FilterExpr::Or {
                items: vec![tag("a"), tag("b")],
            }),
        }),
        "not (a or b)"
    );
}

#[test]
fn single_item_operator_has_no_operator() {
    assert_eq!(
        pretty(&FilterExpr::And {
            items: vec![tag("a")]
        }),
        "a"
    );
    assert_eq!(
        pretty(&FilterExpr::Or {
            items: vec![tag("a")]
        }),
        "a"
    );
}

#[test]
fn quotes_names_with_reserved_characters() {
    assert_eq!(pretty(&tag("a b")), "\"a b\"");
    assert_eq!(pretty(&tag("a\"b")), "\"a\\\"b\"");
    assert_eq!(pretty(&tag("a\\b")), "\"a\\\\b\"");
    assert_eq!(pretty(&tag("")), "\"\"");
}

#[test]
fn round_trips_through_parse() {
    for query in [
        "a",
        "not paye",
        "facture and not paye",
        "(a or b) and c",
        "not (a and b)",
        "a and b and c",
        "a or b and c",
        "only a",
        "not only a",
        "only a or b",
        "exactly (a and b)",
        "not exactly (a and b)",
        "exactly a or b",
    ] {
        let parsed = parse(query).expect("requête valide");
        let printed = pretty(&parsed);
        assert_eq!(
            parse(&printed).expect("forme canonique re-lisible"),
            parsed,
            "requête: {query}, canonique: {printed}"
        );
        assert_eq!(pretty(&parse(&printed).expect("re-parse")), printed);
        assert!(!printed.contains('\n'));
    }
}
