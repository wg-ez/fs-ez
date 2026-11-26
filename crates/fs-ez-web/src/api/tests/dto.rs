use fs_ez_core::TagColor;

#[test]
fn dto_colors_are_palette_tokens() {
    let tag = fs_ez_core::Tag {
        id: fs_ez_core::TagId(1),
        name: "facture".to_string(),
        name_norm: "facture".to_string(),
        color: TagColor::Blue,
        color_custom: None,
        created_at: time::OffsetDateTime::now_utc(),
    };
    let dto = crate::api::dto::TagDto::from_tag(tag, Some(3));
    let json = serde_json::to_value(dto).expect("sérialisation");
    assert_eq!(json["color"], "blue");
    assert_eq!(json["count"], 3);
}
