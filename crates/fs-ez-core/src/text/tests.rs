use super::*;

#[test]
fn unsafe_text_rejects_proto_keys_and_controls() {
    for bad in [
        "__proto__",
        "a[__proto__]",
        "constructor",
        "  Prototype ",
        "a\nb",
        "a\u{7f}b",
        "a\u{85}b",
        "a\u{2028}b",
        "a\u{2029}b",
    ] {
        assert!(has_unsafe_text(bad), "{bad:?}");
    }
    assert!(!has_unsafe_text("Projet client"));
    assert!(!has_unsafe_text("café-été_9"));
    assert!(!has_unsafe_text("constructeur"));
}

#[test]
fn unsafe_text_rejects_bidi_and_invisible_formats() {
    for bad in [
        "tag\u{202E}txt",
        "invoice\u{200B}.exe",
        "a\u{2066}b",
        "\u{FEFF}tag",
        "a\u{00AD}b",
        "a\u{061C}b",
        "a\u{180E}b",
        "a\u{200F}b",
        "a\u{202A}b",
        "a\u{2060}b",
        "a\u{2064}b",
        "a\u{206F}b",
        "a\u{2069}b",
        "a\u{FFF9}b",
        "a\u{FFFB}b",
        "a\u{115F}b",
        "a\u{1160}b",
        "a\u{3164}b",
        "a\u{FFA0}b",
        "a\u{1D173}b",
        "a\u{1D17A}b",
        "a\u{E0000}b",
        "a\u{E0001}b",
        "a\u{E0041}b",
        "a\u{E007F}b",
    ] {
        assert!(has_unsafe_text(bad), "{bad:?}");
    }
    for good in [
        "café",
        "tag-é_1",
        "🚀",
        "e\u{0301}",
        "👨\u{200D}👩\u{200D}👧",
        "❤\u{FE0F}",
    ] {
        assert!(!has_unsafe_text(good), "{good:?}");
    }
}

#[test]
fn sanitize_neutralise_separateurs_et_points() {
    assert_eq!(sanitize_component("a/b\\c\0d\ne"), "a_b_c_d_e");
    assert_eq!(sanitize_component(""), "_");
    assert_eq!(sanitize_component("."), "._");
    assert_eq!(sanitize_component(".."), "_..");
}

#[test]
fn display_safe_remplace_controles_et_formats_bidi() {
    assert_eq!(display_safe("a\u{1b}[31mb"), "a?[31mb");
    assert_eq!(display_safe("tag\u{202E}txt"), "tag?txt");
    assert_eq!(display_safe("a\u{E0041}b"), "a?b");
    assert_eq!(display_safe("a\u{3164}b"), "a?b");
    assert_eq!(display_safe("café 🚀"), "café 🚀");
    assert_eq!(
        display_safe("👨\u{200D}👩\u{200D}👧"),
        "👨\u{200D}👩\u{200D}👧"
    );
    assert_eq!(display_safe("❤\u{FE0F}"), "❤\u{FE0F}");
}
