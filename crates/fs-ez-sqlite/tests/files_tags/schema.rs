use super::*;

#[test]
fn data_version_changes_only_for_writes_from_another_connection() {
    let dir = tempfile::tempdir().expect("repertoire");
    let path = dir.path().join("fs-ez.db");
    let first = SqliteStore::open(&path).expect("premiere connexion");
    let second = SqliteStore::open(&path).expect("seconde connexion");
    let at = datetime!(2026-01-01 00:00 UTC);
    let initial = first.data_version().expect("version initiale");

    insert(&first, "/locale", None, 0, at);
    assert_eq!(
        first.data_version().expect("version locale"),
        initial,
        "une écriture locale ne change pas data_version"
    );

    insert(&second, "/externe", None, 0, at);
    assert_ne!(
        first.data_version().expect("version externe"),
        initial,
        "un commit d'une autre connexion change data_version"
    );
}

#[test]
fn schema_version_reads_temporary_database() {
    let dir = tempfile::tempdir().expect("repertoire");
    let path = dir.path().join("fs-ez.db");
    assert_eq!(SqliteStore::schema_version(&path).expect("version"), 2);
}
