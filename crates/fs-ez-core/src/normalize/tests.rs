use super::*;

#[test]
fn trims_edges_and_keeps_display_case() {
    let (display, norm) = normalize_tag_name("  Projet  ").expect("nom valide");
    assert_eq!(display, "Projet");
    assert_eq!(norm, "projet");
}

#[test]
fn lowers_unicode_without_stripping_accents() {
    let (display, norm) = normalize_tag_name("Été").expect("nom valide");
    assert_eq!(display, "Été");
    assert_eq!(norm, "été");
}

#[test]
fn rejects_empty_whitespace_only_and_control() {
    assert!(normalize_tag_name("   ").is_err());
    assert!(normalize_tag_name("").is_err());
    assert!(normalize_tag_name("a\nb").is_err());
    assert!(normalize_tag_name("a\u{7f}b").is_err());
}

#[test]
fn rejects_proto_keys_and_unicode_separators() {
    for candidate in ["__proto__", "constructor", " Prototype ", "a\u{2028}b"] {
        assert!(
            normalize_tag_name(candidate).is_err(),
            "tag accepté: {candidate:?}"
        );
    }
}

#[test]
fn rejects_reserved_characters() {
    for candidate in [
        "a\tb", "a,b", "a=b", "a+b", "a(b", "a)b", "a\"b", "a'b", "a/b", "a\\b", "a<b", "a>b",
        "a!b", "a&b", "a|b",
    ] {
        let tag = normalize_tag_name(candidate);
        assert!(tag.is_err(), "tag accepté: {candidate:?}");
    }
}

#[test]
fn tag_names_allow_inner_spaces() {
    let (display, norm) = normalize_tag_name("  Projet client  ").expect("nom valide");
    assert_eq!(display, "Projet client");
    assert_eq!(norm, "projet client");
}

#[test]
fn enforces_max_length() {
    let ok = "é".repeat(64);
    assert_eq!(normalize_tag_name(&ok).expect("64 caractères").0, ok);
    let too_long = "é".repeat(65);
    assert!(normalize_tag_name(&too_long).is_err());
}

#[test]
fn error_carries_offending_name() {
    let err = normalize_tag_name("a=b").expect_err("refusé");
    assert!(matches!(err, CoreError::InvalidName(ref name) if name == "a=b"));
}
