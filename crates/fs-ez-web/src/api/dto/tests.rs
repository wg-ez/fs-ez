use super::*;

#[test]
fn page_keeps_offset_and_maps_items() {
    let page = Page {
        items: vec![1u64, 2],
        next_offset: Some(4),
    };
    let dto = PageDto::from_page(page, |item| item.to_string());
    assert_eq!(dto.items, vec!["1", "2"]);
    assert_eq!(dto.next_offset, Some(4));
}

#[test]
fn file_dto_uses_display_path_and_string_fingerprint() {
    use time::macros::datetime;

    let file = FileWithTags {
        file: fs_ez_core::FileRecord {
            id: FileId(4),
            path: std::path::PathBuf::from("/tmp/\u{fffd}"),
            path_display: "/tmp/\u{fffd}".to_string(),
            name: "\u{fffd}".to_string(),
            dir: "/tmp".to_string(),
            size: 12,
            mtime: datetime!(2026-01-02 03:04:05 UTC),
            kind: FileKind::File,
            fingerprint: Some(fs_ez_core::Fingerprint::new("blake3:abc").expect("empreinte")),
            created_at: datetime!(2026-01-02 03:04:05 UTC),
            updated_at: datetime!(2026-01-02 03:04:05 UTC),
        },
        tags: Vec::new(),
    };
    let dto = FileDto::from_file(file);
    assert_eq!(dto.path, "/tmp/\u{fffd}");
    assert_eq!(dto.fingerprint.as_deref(), Some("blake3:abc"));
    let json = serde_json::to_value(&dto).expect("sérialisation");
    assert_eq!(json["id"], 4);
    assert!(json["mtime"].as_str().is_some());
}
