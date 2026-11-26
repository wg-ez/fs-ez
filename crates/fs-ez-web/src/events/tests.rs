use fs_ez_sqlite::SqliteStore;
use fs_ez_store::SettingsStore;
use tempfile::TempDir;

use super::*;
use crate::test_support::persistent_state;

#[test]
fn every_event_maps_to_its_sse_name_and_data() {
    let cases = [
        (Event::DataChanged, "data-changed", json!({ "schema": 1 })),
        (Event::InboxChanged, "inbox-changed", json!({ "schema": 1 })),
        (Event::Resync, "resync", json!({ "schema": 1 })),
        (
            Event::ImportFinished {
                imported: 3,
                duplicates: 2,
            },
            "import-finished",
            json!({ "schema": 1, "imported": 3, "duplicates": 2 }),
        ),
    ];
    for (event, name, data) in cases {
        let frame = frame(&event);
        assert_eq!(frame.name, name);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&frame.data).expect("json"),
            data
        );
    }
}

#[test]
fn hello_carries_schema_version() {
    let frame = hello_frame("1.2.3");
    assert_eq!(frame.name, "hello");
    let data: serde_json::Value = serde_json::from_str(&frame.data).expect("json");
    assert_eq!(data["schema"], 1);
    assert_eq!(data["version"], "1.2.3");
}

#[test]
fn lagged_becomes_resync_and_closed_ends_the_stream() {
    let lagged = received(Err(RecvError::Lagged(4))).expect("resync");
    assert_eq!(lagged.name, "resync");
    assert!(received(Err(RecvError::Closed)).is_none());
}

#[tokio::test]
async fn watcher_publishes_after_a_write_from_another_connection() {
    let dir = TempDir::new().expect("tempdir");
    let state = persistent_state(&dir);
    let second = SqliteStore::open(&dir.path().join("fs-ez.db")).expect("seconde connexion");
    let mut rx = state.events.subscribe();
    let watcher = spawn_data_watcher(state);
    // Laisse le premier tick établir la version de référence.
    tokio::time::sleep(Duration::from_millis(150)).await;
    second
        .set_setting("external.write", "1")
        .expect("écriture externe");
    let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("événement sous 2 s")
        .expect("canal ouvert");
    assert!(matches!(event, Event::DataChanged));
    watcher.abort();
    assert!(watcher.await.is_err());
}
