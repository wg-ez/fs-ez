use rusqlite::Connection;

use super::*;
use crate::db;

#[test]
fn backup_to_writes_a_readable_copy() {
    let temp = tempfile::tempdir().expect("tempdir");
    let store = SqliteStore::from_connection(db::open_in_memory().expect("base"));
    store
        .with_conn(|conn| {
            conn.execute(
                "INSERT INTO setting(key, value) VALUES ('io.backup_days', '3')",
                [],
            )
            .map_err(map_db)?;
            Ok(())
        })
        .expect("insertion");
    let target = temp.path().join("copy.db");

    store.backup_to(&target).expect("copie");
    let copy = Connection::open(target).expect("copie lisible");
    let value: String = copy
        .query_row(
            "SELECT value FROM setting WHERE key = 'io.backup_days'",
            [],
            |row| row.get(0),
        )
        .expect("reglage copie");
    assert_eq!(value, "3");
}

#[test]
fn backup_to_refuses_an_existing_target() {
    let temp = tempfile::tempdir().expect("tempdir");
    let store = SqliteStore::from_connection(db::open_in_memory().expect("base"));
    let target = temp.path().join("copy.db");
    std::fs::write(&target, b"present").expect("fichier");
    assert!(store.backup_to(&target).is_err());
}
