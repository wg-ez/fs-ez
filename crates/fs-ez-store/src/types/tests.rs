use super::*;
use fs_ez_core::{SortDir, SortField};

fn base() -> FileListQuery {
    FileListQuery {
        filter: None,
        untagged: false,
        tagged_since_days: None,
        modified_since_days: None,
        text: None,
        virtual_folder: None,
        sort: Sort {
            field: SortField::Id,
            dir: SortDir::Asc,
        },
        offset: 0,
        limit: 50,
    }
}

fn tag_filter() -> FilterExpr {
    FilterExpr::Tag {
        tag: "facture".to_string(),
        negated: false,
    }
}

#[test]
fn page_limit_bounds_are_enforced() {
    assert!(validate_page_limit(1).is_ok());
    assert!(validate_page_limit(MAX_PAGE_SIZE).is_ok());
    for limit in [0, MAX_PAGE_SIZE + 1] {
        assert!(matches!(
            validate_page_limit(limit),
            Err(StoreError::Invalid(_))
        ));
    }
}

#[test]
fn new_has_empty_filters() {
    assert_eq!(FileListQuery::new(Sort::default(), 50), base());
}

#[test]
fn recent_sets_tagged_since_days_only() {
    let query = FileListQuery::recent(7, Sort::default(), 25);
    assert_eq!(query.tagged_since_days, Some(7));
    assert_eq!(query.modified_since_days, None);
    assert!(!query.untagged);
    assert_eq!(query.filter, None);
    assert_eq!(query.text, None);
    assert_eq!(query.offset, 0);
    assert_eq!(query.limit, 25);
    assert!(query.validate().is_ok());
}

#[test]
fn untagged_sets_untagged_only() {
    let query = FileListQuery::untagged(Sort::default(), 10);
    assert!(query.untagged);
    assert_eq!(query.tagged_since_days, None);
    assert_eq!(query.modified_since_days, None);
    assert_eq!(query.filter, None);
    assert_eq!(query.text, None);
    assert_eq!(query.offset, 0);
    assert_eq!(query.limit, 10);
    assert!(query.validate().is_ok());
}

#[test]
fn default_query_is_valid() {
    assert!(base().validate().is_ok());
}

#[test]
fn recent_files_sets_modified_since_days_only() {
    let query = FileListQuery::recent_files(7, Sort::default(), 25);
    assert_eq!(query.modified_since_days, Some(7));
    assert_eq!(query.tagged_since_days, None);
    assert!(!query.untagged);
    assert!(query.validate().is_ok());
}

#[test]
fn untagged_rejects_tagged_since_days() {
    let query = FileListQuery {
        untagged: true,
        tagged_since_days: Some(7),
        ..base()
    };
    assert!(matches!(query.validate(), Err(StoreError::Invalid(_))));
}

#[test]
fn untagged_rejects_modified_since_days() {
    let query = FileListQuery {
        untagged: true,
        modified_since_days: Some(7),
        ..base()
    };
    assert!(matches!(query.validate(), Err(StoreError::Invalid(_))));
}

#[test]
fn tagged_and_modified_since_days_are_exclusive() {
    let query = FileListQuery {
        tagged_since_days: Some(7),
        modified_since_days: Some(7),
        ..base()
    };
    assert!(matches!(query.validate(), Err(StoreError::Invalid(_))));
}

#[test]
fn untagged_rejects_filter() {
    let query = FileListQuery {
        untagged: true,
        filter: Some(tag_filter()),
        ..base()
    };
    assert!(matches!(query.validate(), Err(StoreError::Invalid(_))));
}

#[test]
fn text_coexists_with_other_predicates() {
    let query = FileListQuery {
        filter: Some(tag_filter()),
        tagged_since_days: Some(3),
        text: Some("abc".to_string()),
        ..base()
    };
    assert!(query.validate().is_ok());
}

#[test]
fn api_token_name_rejects_empty() {
    assert_eq!(normalize_api_token_name(""), None);
}

#[test]
fn api_token_name_rejects_blank() {
    assert_eq!(normalize_api_token_name("   \t\n"), None);
}

#[test]
fn api_token_name_trims() {
    assert_eq!(
        normalize_api_token_name("  ci deploy  "),
        Some("ci deploy".to_string())
    );
}

#[test]
fn api_token_name_accepts_max_length() {
    let raw = "a".repeat(100);
    assert_eq!(normalize_api_token_name(&raw), Some(raw));
}

#[test]
fn api_token_name_rejects_over_max_length() {
    assert_eq!(normalize_api_token_name(&"a".repeat(101)), None);
}

#[test]
fn api_token_name_rejects_unsafe_text() {
    for bad in ["a\nb", "__proto__", "constructor", "a\u{2028}b"] {
        assert_eq!(normalize_api_token_name(bad), None, "{bad:?}");
    }
}

#[test]
fn text_is_bounded_by_characters() {
    let ok = FileListQuery {
        text: Some("é".repeat(MAX_TEXT_LEN)),
        ..base()
    };
    assert!(ok.validate().is_ok());
    let too_long = FileListQuery {
        text: Some("a".repeat(MAX_TEXT_LEN + 1)),
        ..base()
    };
    assert!(matches!(too_long.validate(), Err(StoreError::Invalid(_))));
}
