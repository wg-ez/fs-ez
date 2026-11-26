use super::*;

fn now() -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("timestamp valide")
}

#[test]
fn file_kind_parse_round_trip() {
    for kind in [FileKind::File, FileKind::Directory, FileKind::Symlink] {
        assert_eq!(FileKind::parse(kind.as_str()), Some(kind));
    }
    assert_eq!(FileKind::parse("socket"), None);
}

#[test]
fn fingerprint_rejects_empty_and_too_long() {
    assert!(Fingerprint::new("").is_err());
    assert!(Fingerprint::new("   ").is_err());
    assert!(Fingerprint::new("a".repeat(201)).is_err());
    assert!(Fingerprint::new("a".repeat(200)).is_ok());
}

#[test]
fn fingerprint_trims() {
    let fp = Fingerprint::new("  blake3:abc  ").expect("empreinte valide");
    assert_eq!(fp.as_str(), "blake3:abc");
}

#[test]
fn file_record_serializes_non_utf8_path_lossily() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let raw = OsString::from_vec(vec![b'/', b't', b'm', b'p', b'/', 0xff]);
    let record = FileRecord {
        id: FileId(1),
        path: PathBuf::from(raw),
        path_display: "/tmp/\u{fffd}".to_string(),
        name: "\u{fffd}".to_string(),
        dir: "/tmp".to_string(),
        size: 0,
        mtime: now(),
        kind: FileKind::File,
        fingerprint: None,
        created_at: now(),
        updated_at: now(),
    };
    let json = serde_json::to_string(&record).expect("chemin non-UTF8 sérialisable");
    assert!(json.contains("\"path\":\"/tmp/\u{fffd}\""));
}
