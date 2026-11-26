use super::*;

#[test]
fn resolve_view_filter_rejects_empty_tag() {
    assert!(resolve_view_filter(Some(r#"{"op":"tag","tag":""}"#), None).is_err());
}

#[test]
fn resolve_view_filter_rejects_empty_tag_set() {
    assert!(resolve_view_filter(Some(r#"{"op":"tag_set","tags":[]}"#), None).is_err());
}

#[test]
fn resolve_view_filter_accepts_valid_json() {
    let filter = resolve_view_filter(Some(r#"{"op":"tag","tag":"x"}"#), None)
        .expect("filtre valide")
        .expect("filtre présent");
    assert_eq!(
        filter,
        FilterExpr::Tag {
            tag: "x".to_string(),
            negated: false,
        }
    );
}

#[test]
fn resolve_view_filter_rejects_divergent_query() {
    let result = resolve_view_filter(Some(r#"{"op":"tag","tag":"x"}"#), Some("tag:y"));
    assert!(result.is_err());
}
