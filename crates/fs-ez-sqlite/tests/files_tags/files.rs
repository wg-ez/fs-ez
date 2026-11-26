use super::*;

#[test]
fn insert_lookup_and_conflicts() {
    let store = store();
    let at = datetime!(2026-01-01 00:00 UTC);
    let id = insert(&store, "/data/a.txt", Some("fp-a"), 12, at);
    let record = store.get_file(id).expect("lecture").expect("present");
    assert_eq!(record.path, PathBuf::from("/data/a.txt"));
    assert_eq!(record.name, "a.txt");
    assert_eq!(record.dir, "/data");
    assert_eq!(record.size, 12);
    assert_eq!(
        store
            .find_file_by_path(Path::new("/data/a.txt"))
            .expect("chemin")
            .map(|found| found.id),
        Some(id)
    );
    assert_eq!(
        store
            .find_file_by_name("a.txt")
            .expect("nom")
            .map(|found| found.id),
        Some(id)
    );
    assert_eq!(
        store
            .find_file_by_fingerprint(&Fingerprint::new("fp-a").expect("empreinte"))
            .expect("empreinte")
            .map(|found| found.id),
        Some(id)
    );
    assert!(matches!(
        store.insert_file(&new_file(Path::new("/data/a.txt"), None, 0, at)),
        Err(StoreError::Conflict(_))
    ));
    assert!(matches!(
        store.insert_file(&new_file(Path::new("/data/b.txt"), Some("fp-a"), 0, at)),
        Err(StoreError::Conflict(_))
    ));
    insert(&store, "/data/c.txt", None, 0, at);
    insert(&store, "/data/d.txt", None, 0, at);
    assert_eq!(store.count_files().expect("compte"), 3);
}

#[test]
fn update_is_partial_and_can_clear_fingerprint() {
    let store = store();
    let at = datetime!(2026-01-01 00:00 UTC);
    let id = insert(&store, "/data/a.txt", Some("fp-a"), 12, at);
    let patch = FilePatch {
        path: None,
        path_display: None,
        name: Some("renamed.txt".to_string()),
        dir: None,
        size: Some(99),
        mtime: None,
        kind: None,
        fingerprint: Some(None),
    };
    let updated = store.update_file(id, &patch).expect("mise a jour");
    assert_eq!(updated.name, "renamed.txt");
    assert_eq!(updated.size, 99);
    assert_eq!(updated.path, PathBuf::from("/data/a.txt"));
    assert!(updated.fingerprint.is_none());
    assert!(
        store
            .find_file_by_fingerprint(&Fingerprint::new("fp-a").expect("empreinte"))
            .expect("empreinte")
            .is_none()
    );
    assert!(matches!(
        store.update_file(FileId(999), &patch),
        Err(StoreError::NotFound("file"))
    ));
}

#[test]
fn update_file_path_if_is_a_compare_and_set() {
    let store = store();
    let at = datetime!(2026-01-01 00:00 UTC);
    let id = insert(&store, "/data/a.txt", Some("fp-a"), 12, at);
    let patch = FilePatch {
        path: Some(PathBuf::from("/data/b.txt")),
        path_display: None,
        name: None,
        dir: None,
        size: None,
        mtime: None,
        kind: None,
        fingerprint: None,
    };

    assert!(
        !store
            .update_file_path_if(id, Path::new("/data/stale.txt"), &patch)
            .expect("cas perdu")
    );
    let unchanged = store.get_file(id).expect("lecture").expect("ligne");
    assert_eq!(unchanged.path, PathBuf::from("/data/a.txt"));

    assert!(
        store
            .update_file_path_if(id, Path::new("/data/a.txt"), &patch)
            .expect("cas gagné")
    );
    let updated = store.get_file(id).expect("lecture").expect("ligne");
    assert_eq!(updated.path, PathBuf::from("/data/b.txt"));
    assert_eq!(updated.id, id);
}

#[test]
fn batch_lookup_and_insert_are_grouped() {
    let store = store();
    let at = datetime!(2026-01-01 00:00 UTC);
    let ids = store
        .insert_files_batch(&[
            new_file(Path::new("/data/a.txt"), Some("fp-a"), 1, at),
            new_file(Path::new("/data/b.txt"), Some("fp-b"), 2, at),
        ])
        .expect("lot");
    assert_eq!(ids.len(), 2);
    let found = store
        .find_files_by_fingerprints(&[
            Fingerprint::new("fp-a").expect("empreinte"),
            Fingerprint::new("fp-absent").expect("empreinte"),
        ])
        .expect("empreintes");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, ids[0]);
    let by_path = store
        .find_files_by_paths(&[
            PathBuf::from("/data/a.txt"),
            PathBuf::from("/data/b.txt"),
            PathBuf::from("/data/absent.txt"),
        ])
        .expect("chemins");
    assert_eq!(by_path.len(), 2);
}

#[test]
fn batch_insert_rolls_back_on_unique_conflict() {
    let store = store();
    let at = datetime!(2026-01-01 00:00 UTC);
    let before = store.count_files().expect("compte");
    let result = store.insert_files_batch(&[
        new_file(Path::new("/data/a.txt"), Some("fp-same"), 1, at),
        new_file(Path::new("/data/b.txt"), Some("fp-same"), 2, at),
    ]);
    assert!(matches!(result, Err(StoreError::Conflict(_))));
    assert_eq!(store.count_files().expect("compte"), before);
    assert!(
        store
            .find_file_by_path(Path::new("/data/a.txt"))
            .expect("chemin")
            .is_none()
    );
}

#[test]
fn get_files_by_ids_returns_each_present_row_once() {
    let store = store();
    let at = datetime!(2026-01-01 00:00 UTC);
    let first = insert(&store, "/data/a.txt", None, 0, at);
    let second = insert(&store, "/data/b.txt", None, 0, at);
    let third = insert(&store, "/data/c.txt", None, 0, at);

    let found = store
        .get_files_by_ids(&[third, first, FileId(999), first])
        .expect("lot");

    assert_eq!(
        found.iter().map(|record| record.id).collect::<Vec<_>>(),
        vec![first, third]
    );
    assert_eq!(found.len(), 2);
    assert!(found.iter().all(|record| record.id != second));
}

#[test]
fn delete_files_batch_cascades_tags_and_ignores_absent_ids() {
    let store = store();
    let at = datetime!(2026-01-01 00:00 UTC);
    let first = insert(&store, "/data/a.txt", None, 0, at);
    let second = insert(&store, "/data/b.txt", None, 0, at);
    let kept = insert(&store, "/data/c.txt", None, 0, at);
    let tag = tag(&store, "archive");
    store.apply_tag(first, tag, true, at).expect("tag");

    let deleted = store
        .delete_files_batch(&[first, second, FileId(999)])
        .expect("lot");

    assert_eq!(deleted, 2);
    assert!(store.get_file(first).expect("lecture").is_none());
    assert!(store.get_file(second).expect("lecture").is_none());
    assert!(store.get_file(kept).expect("lecture").is_some());
    assert!(store.tags_for_file(first).expect("tags").is_empty());
}

#[test]
fn delete_files_batch_rolls_back_on_error() {
    let temp = tempfile::TempDir::new().expect("tempdir");
    let state = temp.path().join("state");
    std::fs::create_dir_all(&state).expect("state");
    let db_path = state.join("fs-ez.db");
    let store = SqliteStore::open(&db_path).expect("base");
    let at = datetime!(2026-01-01 00:00 UTC);
    let first = insert(&store, "/data/a.txt", None, 0, at);
    let second = insert(&store, "/data/b.txt", None, 0, at);
    let conn = fs_ez_sqlite::db::open(&db_path).expect("connexion");
    conn.execute(
        &format!(
            "CREATE TRIGGER refuse_delete BEFORE DELETE ON file
             WHEN OLD.id = {} BEGIN SELECT RAISE(ABORT, 'forced'); END",
            second.0
        ),
        [],
    )
    .expect("trigger");

    let result = store.delete_files_batch(&[first, second]);

    assert!(result.is_err());
    assert!(store.get_file(first).expect("lecture").is_some());
    assert!(store.get_file(second).expect("lecture").is_some());
}
