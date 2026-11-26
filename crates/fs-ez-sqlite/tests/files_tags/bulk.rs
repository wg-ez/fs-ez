use super::*;

use fs_ez_sqlite::db;

#[test]
fn add_tags_batch_dedupes_files_and_tags() {
    let store = store();
    let at = datetime!(2026-01-01 00:00 UTC);
    let first = insert(&store, "/a", None, 0, at);
    let second = insert(&store, "/b", None, 0, at);
    let alpha = tag(&store, "alpha");

    let changed = store
        .add_tags_batch(&[first, second, second], &[alpha, alpha], true, at)
        .expect("ajout");

    assert_eq!(changed, 2);
    assert_eq!(store.tags_for_file(first).expect("tags").len(), 1);
    assert_eq!(store.tags_for_file(second).expect("tags").len(), 1);
}

#[test]
fn add_tags_batch_promotes_implicit_rows() {
    let store = store();
    let at = datetime!(2026-01-01 00:00 UTC);
    let file = insert(&store, "/a", None, 0, at);
    let alpha = tag(&store, "alpha");
    store.apply_tag(file, alpha, false, at).expect("implicite");

    let changed = store
        .add_tags_batch(&[file], &[alpha], true, at)
        .expect("promotion");
    let applied = store.tags_for_file(file).expect("tags");

    assert_eq!(changed, 1);
    assert_eq!(applied.len(), 1);
    assert!(applied[0].explicit);
}

#[test]
fn remove_tags_batch_ignores_absent_associations() {
    let store = store();
    let at = datetime!(2026-01-01 00:00 UTC);
    let first = insert(&store, "/a", None, 0, at);
    let second = insert(&store, "/b", None, 0, at);
    let alpha = tag(&store, "alpha");
    store.apply_tag(first, alpha, true, at).expect("alpha");

    let removed = store
        .remove_tags_batch(&[first, second], &[alpha])
        .expect("retrait");

    assert_eq!(removed, 1);
    assert!(store.tags_for_file(first).expect("tags").is_empty());
}

#[test]
fn replace_tags_batch_clears_previous_tags() {
    let store = store();
    let at = datetime!(2026-01-01 00:00 UTC);
    let first = insert(&store, "/a", None, 0, at);
    let second = insert(&store, "/b", None, 0, at);
    let alpha = tag(&store, "alpha");
    let beta = tag(&store, "beta");
    let gamma = tag(&store, "gamma");
    store.apply_tag(first, alpha, true, at).expect("alpha");
    store.apply_tag(first, beta, true, at).expect("beta");
    store
        .apply_tag(second, alpha, true, at)
        .expect("autre fichier");

    store
        .replace_tags_batch(&[first], &[gamma], at)
        .expect("remplacement");

    let applied = store.tags_for_file(first).expect("tags");
    assert_eq!(applied.len(), 1);
    assert_eq!(applied[0].tag.id, gamma);
    assert_eq!(store.tags_for_file(second).expect("tags").len(), 1);
}

#[test]
fn batch_with_empty_selection_is_a_noop() {
    let store = store();
    let at = datetime!(2026-01-01 00:00 UTC);
    let alpha = tag(&store, "alpha");

    assert_eq!(
        store
            .add_tags_batch(&[], &[alpha], true, at)
            .expect("ajout vide"),
        0
    );
    assert_eq!(
        store
            .remove_tags_batch(&[], &[alpha])
            .expect("retrait vide"),
        0
    );
    assert_eq!(
        store
            .replace_tags_batch(&[], &[alpha], at)
            .expect("remplacement vide"),
        0
    );
}

#[test]
fn replace_tags_batch_rolls_back_on_insert_failure() {
    let conn = db::open_in_memory().expect("base");
    conn.execute_batch(
        "CREATE TRIGGER refuse BEFORE INSERT ON file_tag
         WHEN NEW.tag_id = 2 BEGIN SELECT RAISE(ABORT, 'refus'); END;",
    )
    .expect("trigger");
    let store = SqliteStore::from_connection(conn);
    let at = datetime!(2026-01-01 00:00 UTC);
    let file = insert(&store, "/a", None, 0, at);
    let alpha = tag(&store, "alpha");
    let beta = store
        .get_or_create_tag(
            store.list_groups().expect("groupes")[0].id,
            "beta",
            TagColor::Neutral,
        )
        .expect("beta")
        .id;
    store.apply_tag(file, alpha, true, at).expect("alpha");

    let error = store
        .replace_tags_batch(&[file], &[beta], at)
        .expect_err("refus du trigger");
    assert!(matches!(error, StoreError::Conflict(_) | StoreError::Db(_)));

    let applied = store.tags_for_file(file).expect("tags");
    assert_eq!(applied.len(), 1);
    assert_eq!(applied[0].tag.id, alpha);
}

#[test]
fn delete_file_cascades_batch_tags() {
    let store = store();
    let at = datetime!(2026-01-01 00:00 UTC);
    let file = insert(&store, "/a", None, 0, at);
    let alpha = tag(&store, "alpha");
    let beta = tag(&store, "beta");
    store
        .add_tags_batch(&[file], &[alpha, beta], true, at)
        .expect("ajout");

    assert!(store.delete_file(file).expect("suppression"));
    assert!(store.tags_for_file(file).expect("tags").is_empty());
}

#[test]
fn update_files_batch_updates_every_row() {
    let store = store();
    let at = datetime!(2026-01-01 00:00 UTC);
    let first = insert(&store, "/a", None, 0, at);
    let second = insert(&store, "/b", None, 0, at);
    let patch = FilePatch {
        path: None,
        path_display: None,
        name: Some("renomme".to_string()),
        dir: None,
        size: Some(42),
        mtime: None,
        kind: None,
        fingerprint: None,
    };

    let updated = store
        .update_files_batch(&[(first, patch.clone()), (second, patch)])
        .expect("lot");

    assert_eq!(updated, 2);
    for id in [first, second] {
        let record = store.get_file(id).expect("lecture").expect("connu");
        assert_eq!(record.name, "renomme");
        assert_eq!(record.size, 42);
    }
}

#[test]
fn update_files_batch_rolls_back_on_missing_row() {
    let store = store();
    let at = datetime!(2026-01-01 00:00 UTC);
    let file = insert(&store, "/a", None, 0, at);
    let missing = FileId(9999);
    let patch = FilePatch {
        path: None,
        path_display: None,
        name: Some("jamais".to_string()),
        dir: None,
        size: None,
        mtime: None,
        kind: None,
        fingerprint: None,
    };

    let result = store.update_files_batch(&[(file, patch.clone()), (missing, patch)]);

    assert!(matches!(result, Err(StoreError::NotFound(_))));
    let record = store.get_file(file).expect("lecture").expect("connu");
    assert_ne!(record.name, "jamais");
}
