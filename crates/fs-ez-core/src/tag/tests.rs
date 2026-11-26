use super::*;

#[test]
fn all_has_twelve_tokens_in_plan_order() {
    assert_eq!(TagColor::ALL.len(), 12);
    let tokens: Vec<&str> = TagColor::ALL.iter().map(TagColor::as_token).collect();
    assert_eq!(
        tokens,
        [
            "neutral", "red", "orange", "amber", "green", "teal", "cyan", "blue", "indigo",
            "violet", "pink", "brown"
        ]
    );
}

#[test]
fn tag_color_parse_round_trip_and_serde() {
    for color in TagColor::ALL {
        assert_eq!(TagColor::parse(color.as_token()), Some(color));
        let json = serde_json::to_string(&color).expect("sérialisation");
        assert_eq!(json, format!("\"{}\"", color.as_token()));
        let back: TagColor = serde_json::from_str(&json).expect("désérialisation");
        assert_eq!(back, color);
    }
    assert_eq!(TagColor::parse("magenta"), None);
}

#[test]
fn hex_color_accepts_short_and_long_forms() {
    assert_eq!(
        validate_hex_color("#AABBCC").expect("6 chiffres"),
        "#aabbcc"
    );
    assert_eq!(validate_hex_color("#a1B").expect("3 chiffres"), "#aa11bb");
    assert_eq!(validate_hex_color("#000000").expect("noir"), "#000000");
}

#[test]
fn hex_color_rejects_everything_else() {
    for raw in [
        "",
        "#",
        "#12",
        "#1234",
        "#12345",
        "#1234567",
        "aabbcc",
        "#gggggg",
        "#12 34 56",
        " #aabbcc",
        "#aabbcc ",
        "#+12345",
    ] {
        let err = validate_hex_color(raw).expect_err(raw);
        assert!(matches!(err, CoreError::InvalidColor(ref v) if v == raw));
    }
}
