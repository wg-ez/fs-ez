//! Tests de la normalisation des chemins relatifs.

use super::*;

#[test]
fn accepts_plain_segments() {
    assert_eq!(
        normalize_relative_path("  a/b  ").expect("chemin valide"),
        PathBuf::from("a/b")
    );
    assert_eq!(
        normalize_relative_path("a").expect("chemin valide"),
        PathBuf::from("a")
    );
}

#[test]
fn rejects_unsafe_segments() {
    for raw in [
        "", "   ", "/abs", "..", "a/../b", "a//b", "a/./b", ".", "a\0b",
    ] {
        assert!(
            normalize_relative_path(raw).is_none(),
            "accepté à tort: {raw:?}"
        );
    }
}
