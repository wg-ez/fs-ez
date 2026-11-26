//! Tests de la normalisation et des chemins non-UTF8.

use super::*;

#[test]
fn normalize_relative_accepts_plain_segments() {
    assert_eq!(
        normalize_relative("  a/b  ").expect("chemin valide"),
        PathBuf::from("a/b")
    );
    assert_eq!(
        normalize_relative("a").expect("chemin valide"),
        PathBuf::from("a")
    );
}

#[test]
fn normalize_relative_rejects_unsafe_segments() {
    for raw in [
        "", "   ", "/abs", "..", "a/../b", "a//b", "a/./b", ".", "a\0b",
    ] {
        assert!(normalize_relative(raw).is_err(), "accepté à tort: {raw:?}");
    }
}

#[test]
fn non_utf8_bytes_round_trip() {
    let bytes = vec![b'/', b't', b'm', b'p', b'/', 0xff, b'x'];
    let path = path_from_bytes(bytes.clone());
    assert_eq!(path_bytes(&path), bytes.as_slice());
    assert_eq!(path_from_bytes(path_bytes(&path).to_vec()), path);
}

#[test]
fn lossy_helpers_survive_invalid_utf8() {
    let path = path_from_bytes(vec![b'/', b'a', 0xff, b'b']);
    assert_eq!(display_lossy(&path), "/a\u{fffd}b");
    assert_eq!(file_name_lossy(&path).as_deref(), Some("a\u{fffd}b"));
    assert_eq!(dir_lossy(&path).as_deref(), Some("/"));
    assert_eq!(file_name_lossy(Path::new("/")), None);
}
