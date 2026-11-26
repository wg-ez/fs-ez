use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use super::*;

#[test]
fn components_are_deterministic_and_two_hex_levels() {
    let name = OsStr::new("facture.pdf");
    let first = shard_components(name);
    let second = shard_components(name);
    assert_eq!(first, second);
    for component in first {
        assert_eq!(component.len(), 2);
        assert!(component.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(component, component.to_lowercase());
    }
}

#[test]
fn same_name_gets_same_shard_and_different_names_vary() {
    let files = Path::new("/data/files");
    let [first, second] = shard_components(OsStr::new("photo.png"));
    assert_eq!(
        sharded_path(files, OsStr::new("photo.png")),
        files.join(first).join(second).join("photo.png")
    );
    let mut seen = std::collections::HashSet::new();
    for name in ["a.txt", "b.txt", "photo.png", "Photo.png", "facture.pdf"] {
        seen.insert(shard_dir(files, OsStr::new(name)));
    }
    assert!(seen.len() > 1);
}

#[test]
fn accents_dots_and_non_utf8_names_are_hashed_as_bytes() {
    let files = Path::new("/data/files");
    let accented = shard_components(OsStr::new("été 2026.tar.gz"));
    assert_eq!(accented.len(), 2);
    let non_utf8 = OsStr::from_bytes(b"caf\xe9.txt");
    let path = sharded_path(files, non_utf8);
    assert_eq!(path.file_name(), Some(non_utf8));
    let [first, second] = shard_components(non_utf8);
    assert_eq!(path, files.join(first).join(second).join(non_utf8));
}
