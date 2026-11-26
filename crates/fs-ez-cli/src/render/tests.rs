use super::*;

#[test]
fn long_paths_are_truncated_with_ellipsis() {
    let short = truncate_path("/tmp/a");
    assert_eq!(short, "/tmp/a");
    let long = truncate_path(&"a".repeat(120));
    assert_eq!(long.chars().count(), MAX_PATH_CHARS);
    assert!(long.ends_with('…'));
}

#[test]
fn table_has_uppercase_header_and_no_trailing_spaces() {
    let text = table(&["ID", "NAME"], &[vec!["1".to_string(), "a".to_string()]]);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "ID  NAME");
    assert_eq!(lines[1], "1   a");
    assert!(lines.iter().all(|line| !line.ends_with(' ')));
}

#[test]
fn config_text_lists_pairs() {
    let text = config_text(&[("ui.recent_days".to_string(), "3".to_string())]);
    assert!(text.contains("ui.recent_days"));
    assert!(text.contains('3'));
}

fn sample_file(path_display: &str, tag_name: &str) -> FileWithTags {
    let now = fs_ez_core::datetime::now_local();
    FileWithTags {
        file: fs_ez_core::FileRecord {
            id: fs_ez_core::FileId(1),
            path: std::path::PathBuf::from(path_display),
            path_display: path_display.to_string(),
            name: path_display.to_string(),
            dir: String::new(),
            size: 0,
            mtime: now,
            kind: fs_ez_core::FileKind::File,
            fingerprint: None,
            created_at: now,
            updated_at: now,
        },
        tags: vec![AppliedTag {
            tag: Tag {
                id: fs_ez_core::TagId(2),
                name: tag_name.to_string(),
                name_norm: tag_name.to_string(),
                color: TagColor::Blue,
                color_custom: None,
                created_at: now,
            },
            explicit: true,
            created_at: now,
        }],
    }
}

#[test]
fn file_table_replaces_control_and_bidi_characters() {
    let files = [sample_file("/tmp/\u{1b}[31mred", "tag\u{202E}x")];
    let text = file_table(&files);
    assert!(!text.contains('\u{1b}'), "{text:?}");
    assert!(!text.contains('\u{202E}'), "{text:?}");
    assert!(text.contains("?[31mred"), "{text:?}");
    assert!(text.contains("tag?x"), "{text:?}");
}

#[test]
fn file_table_keeps_safe_unicode_names() {
    let files = [sample_file("/tmp/café 🚀", "café")];
    let text = file_table(&files);
    assert!(text.contains("/tmp/café 🚀"), "{text:?}");
    assert!(text.contains("café"), "{text:?}");
    assert!(!text.contains('?'), "{text:?}");
}
