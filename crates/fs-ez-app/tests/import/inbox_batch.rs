use super::*;

#[test]
fn scan_inbox_collision_gets_a_suffix_and_both_are_indexed() {
    let harness = Harness::with_inbox();
    let inbox = harness.inbox_path();
    let first = inbox.join("a");
    let second = inbox.join("b");
    std::fs::create_dir_all(&first).expect("a");
    std::fs::create_dir_all(&second).expect("b");
    std::fs::write(first.join("photo.png"), b"premier").expect("photo a");
    std::fs::write(second.join("photo.png"), b"second").expect("photo b");

    let counts = harness.app.scan_inbox().expect("scan inbox");
    assert_eq!(counts, import_counts(2, 0, 0));
    let files = harness.app.files_path().expect("files");
    let mut contents = files_under(&files)
        .into_iter()
        .map(|path| std::fs::read(path).expect("contenu"))
        .collect::<Vec<_>>();
    contents.sort();
    assert_eq!(contents, vec![b"premier".to_vec(), b"second".to_vec()]);
    assert!(known(&harness.app, "photo.png"));
}

#[test]
fn scan_inbox_batch_imports_every_file_and_preserves_metadata() {
    let harness = Harness::with_inbox();
    let inbox = harness.inbox_path();
    let mut expected = Vec::new();
    for index in 0..5u64 {
        let name = format!("lot-{index}.txt");
        let path = inbox.join(&name);
        std::fs::write(&path, format!("contenu-{index}")).expect("dépôt");
        let stamp = UNIX_EPOCH + Duration::from_secs(1_700_000_000 + index);
        set_modified(&path, stamp);
        let inode = std::fs::metadata(&path).expect("meta source").ino();
        expected.push((name, stamp, inode));
    }

    let counts = harness.app.scan_inbox().expect("scan inbox");
    assert_eq!(counts, import_counts(5, 0, 0));
    let files = harness.app.files_path().expect("files");
    for (name, stamp, inode) in expected {
        let source = inbox.join(&name);
        let stored = sharded_path(&files, OsStr::new(&name));
        assert!(!source.exists(), "source consommée");
        assert_eq!(
            std::fs::metadata(&stored).expect("meta").ino(),
            inode,
            "lien matériel attendu"
        );
        assert_eq!(modified(&stored), stamp);
        assert!(known(&harness.app, &name));
    }
}

#[test]
fn scan_inbox_same_batch_duplicate_imports_one_file() {
    let harness = Harness::with_inbox();
    let inbox = harness.inbox_path();
    std::fs::write(inbox.join("premier.txt"), b"identique").expect("premier");
    std::fs::write(inbox.join("second.txt"), b"identique").expect("second");

    let counts = harness.app.scan_inbox().expect("scan inbox");
    assert_eq!(counts, import_counts(1, 1, 0));
    assert!(!inbox.join("premier.txt").exists());
    assert!(!inbox.join("second.txt").exists());
    let files = harness.app.files_path().expect("files");
    let stored = std::fs::read_dir(&files)
        .expect("files")
        .flat_map(|shard| {
            std::fs::read_dir(shard.expect("shard").path())
                .expect("shard")
                .collect::<Vec<_>>()
        })
        .count();
    assert_eq!(
        stored, 1,
        "une seule destination pour deux sources identiques"
    );
}

#[test]
fn scan_inbox_batch_insert_failure_falls_back_and_keeps_sources() {
    let temp = TempDir::new().expect("racine");
    let app = app_with_failing_inbox_trigger(&temp);
    let inbox = temp.path().join("inbox");
    std::fs::create_dir_all(&inbox).expect("inbox");
    std::fs::write(inbox.join("boom.txt"), b"boom").expect("boom");
    std::fs::write(inbox.join("ok.txt"), b"ok").expect("ok");

    let counts = app.scan_inbox().expect("scan inbox");
    assert_eq!(counts, import_counts(1, 0, 1));
    assert!(
        inbox.join("boom.txt").exists(),
        "source conservée après échec"
    );
    assert!(!stored_path(&app, "boom.txt").exists());
    assert!(missing(&app, "boom.txt"));
    assert!(!inbox.join("ok.txt").exists(), "repli unitaire réussi");
    assert_eq!(
        std::fs::read(stored_path(&app, "ok.txt")).expect("ok stocké"),
        b"ok"
    );
    assert!(known(&app, "ok.txt"));
}

#[test]
fn scan_inbox_fallback_keeps_folder_of_deferred_duplicate() {
    let temp = TempDir::new().expect("racine");
    let app = app_with_failing_inbox_trigger(&temp);
    let inbox = temp.path().join("inbox");
    std::fs::create_dir_all(inbox.join("sous")).expect("inbox");
    std::fs::write(inbox.join("boom.txt"), b"identique").expect("boom");
    std::fs::write(inbox.join("sous/doublon.txt"), b"identique").expect("doublon");

    let counts = app.scan_inbox().expect("scan inbox");
    assert_eq!(counts, import_counts(1, 0, 1));
    let record = app
        .find_by_name("doublon.txt")
        .expect("base")
        .expect("fichier importé au repli");
    let folder = app
        .store
        .folder_of_file(record.id)
        .expect("dossier")
        .expect("dossier virtuel attendu");
    assert_eq!(
        app.get_virtual_folder(folder).expect("lecture").name,
        "sous",
        "dossier d'origine conservé au repli"
    );
}
