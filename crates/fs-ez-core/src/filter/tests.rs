use super::*;

fn tag(name: &str) -> FilterExpr {
    FilterExpr::Tag {
        tag: name.to_string(),
        negated: false,
    }
}

fn round_trip(expr: &FilterExpr) {
    let json = serde_json::to_string(expr).expect("sérialisation");
    let back: FilterExpr = serde_json::from_str(&json).expect("désérialisation");
    assert_eq!(&back, expr);
}

#[test]
fn serde_round_trip_all_variants() {
    round_trip(&tag("facture"));
    round_trip(&FilterExpr::Tag {
        tag: "statut".to_string(),
        negated: true,
    });
    round_trip(&FilterExpr::And {
        items: vec![tag("a"), tag("b")],
    });
    round_trip(&FilterExpr::Or {
        items: vec![tag("a"), tag("b")],
    });
    round_trip(&FilterExpr::OnlyTag {
        tag: "facture".to_string(),
        negated: false,
    });
    round_trip(&FilterExpr::OnlyTag {
        tag: "facture".to_string(),
        negated: true,
    });
    round_trip(&FilterExpr::TagSet {
        tags: vec!["network".to_string(), "software".to_string()],
        negated: false,
    });
    round_trip(&FilterExpr::TagSet {
        tags: vec!["network".to_string()],
        negated: true,
    });
    round_trip(&FilterExpr::Not {
        item: Box::new(tag("a")),
    });
}

#[test]
fn serialized_shape_uses_op_tag() {
    let json = serde_json::to_string(&FilterExpr::Tag {
        tag: "t".to_string(),
        negated: false,
    })
    .expect("sérialisation");
    assert_eq!(json, r#"{"op":"tag","tag":"t","negated":false}"#);
}

#[test]
fn without_tag_prunes_nested_references_and_collapses() {
    let expr = FilterExpr::And {
        items: vec![
            tag("A"),
            FilterExpr::Not {
                item: Box::new(tag("b")),
            },
            FilterExpr::TagSet {
                tags: vec!["a".to_string(), "c".to_string()],
                negated: false,
            },
            FilterExpr::Or {
                items: vec![tag("a"), tag("d")],
            },
        ],
    };
    let pruned = expr.without_tag("a").expect("filtre restant");
    assert_eq!(
        pruned,
        FilterExpr::And {
            items: vec![
                FilterExpr::Not {
                    item: Box::new(tag("b")),
                },
                FilterExpr::TagSet {
                    tags: vec!["c".to_string()],
                    negated: false,
                },
                tag("d"),
            ],
        }
    );
}

#[test]
fn without_tag_returns_none_when_nothing_remains() {
    assert!(tag("seul").without_tag("seul").is_none());
    assert!(
        FilterExpr::OnlyTag {
            tag: "Seul".to_string(),
            negated: true,
        }
        .without_tag("seul")
        .is_none()
    );
    assert!(
        FilterExpr::TagSet {
            tags: vec!["seul".to_string()],
            negated: false,
        }
        .without_tag("seul")
        .is_none()
    );
    assert!(
        FilterExpr::And {
            items: vec![tag("seul"), tag("seul")],
        }
        .without_tag("seul")
        .is_none()
    );
}

#[test]
fn required_tags_unions_positive_constraints() {
    let expr = FilterExpr::And {
        items: vec![
            tag("Work"),
            FilterExpr::And {
                items: vec![tag("archive"), tag("work")],
            },
            FilterExpr::TagSet {
                tags: vec!["projet".to_string(), "archive".to_string()],
                negated: false,
            },
            FilterExpr::OnlyTag {
                tag: "seul".to_string(),
                negated: false,
            },
        ],
    };
    assert_eq!(
        expr.required_tags(),
        [
            "Work".to_string(),
            "archive".to_string(),
            "projet".to_string(),
            "seul".to_string()
        ]
    );
}

#[test]
fn required_tags_ignores_negated_alternatives_and_exclusions() {
    for expr in [
        FilterExpr::Tag {
            tag: "a".to_string(),
            negated: true,
        },
        FilterExpr::Or {
            items: vec![tag("a"), tag("b")],
        },
        FilterExpr::Not {
            item: Box::new(tag("a")),
        },
        FilterExpr::And {
            items: vec![
                FilterExpr::Tag {
                    tag: "a".to_string(),
                    negated: true,
                },
                FilterExpr::Not {
                    item: Box::new(tag("b")),
                },
            ],
        },
    ] {
        assert!(expr.required_tags().is_empty(), "{expr:?}");
    }
}

#[test]
fn validate_rejects_empty_and_or() {
    assert!(FilterExpr::And { items: vec![] }.validate().is_err());
    assert!(FilterExpr::Or { items: vec![] }.validate().is_err());
    assert!(tag("a").validate().is_ok());
}

#[test]
fn validate_rejects_empty_tag() {
    assert!(tag("   ").validate().is_err());
    assert!(
        FilterExpr::OnlyTag {
            tag: "   ".to_string(),
            negated: false,
        }
        .validate()
        .is_err()
    );
    assert!(
        FilterExpr::TagSet {
            tags: Vec::new(),
            negated: false,
        }
        .validate()
        .is_err()
    );
    assert!(
        FilterExpr::TagSet {
            tags: vec!["a".to_string(), "  ".to_string()],
            negated: false,
        }
        .validate()
        .is_err()
    );
}

#[test]
fn validate_rejects_names_outside_the_tag_charset() {
    for bad in ["a=b", "a/b", "__proto__", "a\nb"] {
        assert!(tag(bad).validate().is_err(), "tag accepté: {bad:?}");
        assert!(
            FilterExpr::TagSet {
                tags: vec![bad.to_string()],
                negated: false,
            }
            .validate()
            .is_err(),
            "tag set accepté: {bad:?}"
        );
    }
}

#[test]
fn validate_rejects_too_deep() {
    let mut expr = tag("a");
    for _ in 0..MAX_DEPTH {
        expr = FilterExpr::Not {
            item: Box::new(expr),
        };
    }
    assert!(expr.validate().is_err());
}

#[test]
fn validate_rejects_too_many_nodes() {
    let expr = FilterExpr::And {
        items: (0..MAX_NODES).map(|i| tag(&format!("t{i}"))).collect(),
    };
    assert!(expr.validate().is_err());
}
