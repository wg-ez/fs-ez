//! Tests des conversions de chemins vers les colonnes SQLite.

use super::*;

#[test]
fn name_and_dir_fallbacks() {
    assert_eq!(name_of(Path::new("/")), "/");
    assert_eq!(dir_of(Path::new("/")), "");
    assert_eq!(name_of(Path::new("/a/b")), "b");
    assert_eq!(dir_of(Path::new("/a/b")), "/a");
}

#[test]
fn negative_counts_are_rejected() {
    assert!(matches!(
        to_count(-3),
        Err(StoreError::Db(message)) if message == "negative count -3"
    ));
    assert!(matches!(to_count(0), Ok(0)));
    assert!(matches!(row_count(7), Ok(7)));
}
