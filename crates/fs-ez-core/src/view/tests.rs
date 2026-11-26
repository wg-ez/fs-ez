use super::*;

#[test]
fn defaults() {
    assert_eq!(
        Sort::default(),
        Sort {
            field: SortField::Id,
            dir: SortDir::Asc
        }
    );
}

#[test]
fn sort_serde_round_trip() {
    for field in [
        SortField::Id,
        SortField::Name,
        SortField::Size,
        SortField::Mtime,
    ] {
        for dir in [SortDir::Asc, SortDir::Desc] {
            let sort = Sort { field, dir };
            let json = serde_json::to_string(&sort).expect("sérialisation");
            let back: Sort = serde_json::from_str(&json).expect("désérialisation");
            assert_eq!(back, sort);
        }
    }
}

#[test]
fn enum_tokens_are_lowercase() {
    assert_eq!(
        serde_json::to_string(&SortField::Mtime).expect("sérialisation"),
        "\"mtime\""
    );
    assert_eq!(
        serde_json::to_string(&SortDir::Desc).expect("sérialisation"),
        "\"desc\""
    );
}

#[test]
fn normalize_view_name_rejects_empty() {
    assert_eq!(normalize_view_name(""), None);
}

#[test]
fn normalize_view_name_rejects_blank() {
    assert_eq!(normalize_view_name("   \t\n"), None);
}

#[test]
fn normalize_view_name_accepts_max_length() {
    let raw = "a".repeat(100);
    assert_eq!(normalize_view_name(&raw), Some(raw));
}

#[test]
fn normalize_view_name_rejects_over_max_length() {
    assert_eq!(normalize_view_name(&"a".repeat(101)), None);
}

#[test]
fn normalize_view_name_trims() {
    assert_eq!(
        normalize_view_name("  Mes fichiers  "),
        Some("Mes fichiers".to_string())
    );
}

#[test]
fn normalize_view_name_rejects_unsafe_text() {
    for bad in ["a\nb", "__proto__", "constructor", "a\u{2028}b"] {
        assert_eq!(normalize_view_name(bad), None, "{bad:?}");
    }
}

#[test]
fn normalize_view_icon_accepts_empty_and_rejects_unsafe_or_long() {
    assert_eq!(normalize_view_icon("   ").ok(), Some(None));
    assert_eq!(
        normalize_view_icon("  ◆  ").ok(),
        Some(Some("◆".to_string()))
    );
    assert!(normalize_view_icon("a\nb").is_err());
    assert!(normalize_view_icon("__proto__").is_err());
    assert!(normalize_view_icon(&"a".repeat(17)).is_err());
}

#[test]
fn normalize_group_name_rejects_empty() {
    assert_eq!(normalize_group_name(""), None);
}

#[test]
fn normalize_group_name_rejects_blank() {
    assert_eq!(normalize_group_name("   \t\n"), None);
}

#[test]
fn normalize_group_name_accepts_max_length() {
    let raw = "a".repeat(64);
    assert_eq!(normalize_group_name(&raw), Some(raw));
}

#[test]
fn normalize_group_name_rejects_over_max_length() {
    assert_eq!(normalize_group_name(&"a".repeat(65)), None);
}

#[test]
fn normalize_group_name_rejects_control_chars() {
    assert_eq!(normalize_group_name("Groupe\u{7}"), None);
}

#[test]
fn normalize_group_name_rejects_unsafe_text() {
    for bad in ["__proto__", "constructor", "Groupe\u{2029}fin"] {
        assert_eq!(normalize_group_name(bad), None, "{bad:?}");
    }
}

#[test]
fn normalize_group_name_trims() {
    assert_eq!(
        normalize_group_name("  Travail  "),
        Some("Travail".to_string())
    );
}
