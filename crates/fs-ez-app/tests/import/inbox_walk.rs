use super::*;

#[test]
fn scan_inbox_imports_shard() {
    let harness = Harness::with_inbox();
    let inbox = harness.inbox_path();
    std::fs::write(inbox.join("depot.txt"), b"depot").expect("dépôt");

    let counts = harness.app.scan_inbox().expect("scan inbox");
    assert_eq!(counts, import_counts(1, 0, 0));
    let stored = stored_path(&harness.app, "depot.txt");
    assert_eq!(std::fs::read(&stored).expect("fichier"), b"depot");
    assert!(!inbox.join("depot.txt").exists());
    assert!(known(&harness.app, "depot.txt"));
}

#[test]
fn scan_inbox_preserves_source_mtime() {
    let harness = Harness::with_inbox();
    let inbox = harness.inbox_path();
    let path = inbox.join("date.txt");
    std::fs::write(&path, b"date").expect("dépôt");
    let stamp = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
    set_modified(&path, stamp);

    harness.app.scan_inbox().expect("scan inbox");
    let stored = stored_path(&harness.app, "date.txt");
    assert_eq!(modified(&stored), stamp);
    let record = harness
        .app
        .find_by_name("date.txt")
        .expect("base")
        .expect("fichier");
    assert_eq!(record.mtime.unix_timestamp(), 1_700_000_000);
}

#[test]
fn scan_inbox_walks_subdirs_and_ignores_hidden_and_temporary_names() {
    let harness = Harness::with_inbox();
    let inbox = harness.inbox_path();
    let nested = inbox.join("photos/2026");
    std::fs::create_dir_all(&nested).expect("dossier");
    std::fs::write(nested.join("visible.txt"), b"visible").expect("visible");
    std::fs::write(inbox.join(".cache"), b"cache").expect("cache");
    std::fs::write(inbox.join("partiel.part"), b"part").expect("partiel");
    std::fs::write(nested.join(".hidden"), b"hidden").expect("hidden");
    std::fs::write(nested.join("autre.tmp"), b"tmp").expect("tmp");

    let counts = harness.app.scan_inbox().expect("scan inbox");
    assert_eq!(counts, import_counts(1, 0, 0));
    assert!(known(&harness.app, "visible.txt"));
    assert!(inbox.join(".cache").exists(), "caché jamais déplacé");
    assert!(
        inbox.join("partiel.part").exists(),
        "temporaire jamais déplacé"
    );
    assert!(nested.join(".hidden").exists());
    assert!(nested.join("autre.tmp").exists());
}

#[test]
fn scan_inbox_stops_after_sixteen_levels() {
    let harness = Harness::with_inbox();
    let inbox = harness.inbox_path();
    let mut allowed = inbox.clone();
    for level in 0..16 {
        allowed.push(format!("d{level:02}"));
    }
    std::fs::create_dir_all(&allowed).expect("profondeur 16");
    std::fs::write(allowed.join("ok.txt"), b"ok").expect("ok");
    let mut refused = inbox.clone();
    for level in 0..17 {
        refused.push(format!("e{level:02}"));
    }
    std::fs::create_dir_all(&refused).expect("profondeur 17");
    std::fs::write(refused.join("trop.txt"), b"trop").expect("trop");

    let counts = harness.app.scan_inbox().expect("scan inbox");
    assert_eq!(counts.imported, 1);
    assert!(known(&harness.app, "ok.txt"));
    assert!(missing(&harness.app, "trop.txt"));
    assert!(refused.join("trop.txt").exists());
}

#[test]
fn scan_inbox_drops_duplicate_copy_and_keeps_existing_file() {
    let harness = Harness::with_inbox();
    let inbox = harness.inbox_path();
    std::fs::write(inbox.join("original.txt"), b"identique").expect("original");
    harness.app.scan_inbox().expect("premier scan");
    let stored = stored_path(&harness.app, "original.txt");

    std::fs::write(inbox.join("copie.txt"), b"identique").expect("copie");
    let counts = harness.app.scan_inbox().expect("second scan");
    assert_eq!(counts, import_counts(0, 1, 0));
    assert!(!inbox.join("copie.txt").exists());
    assert_eq!(std::fs::read(&stored).expect("original"), b"identique");
    assert!(missing(&harness.app, "copie.txt"));
}

#[test]
fn scan_inbox_imports_sampled_collision_whose_middle_differs() {
    use fs_ez_fingerprint::Blake3Fingerprinter;

    let temp = tempfile::TempDir::new().expect("racine");
    std::fs::create_dir_all(temp.path().join("files")).expect("files");
    // Seuils réduits : même collision d'échantillon sans écrire 20 Mio.
    let fingerprinter = Arc::new(Blake3Fingerprinter::new(1024, 256).expect("configuration"));
    let app = app_with_fingerprinter(&temp, fingerprinter);
    let inbox = temp.path().join("inbox");
    std::fs::create_dir_all(&inbox).expect("inbox");
    let base = vec![7u8; 4096];
    let mut different = base.clone();
    different[2048] = 8;
    std::fs::write(inbox.join("a.bin"), &base).expect("a");
    std::fs::write(inbox.join("b.bin"), &different).expect("b");
    std::fs::write(inbox.join("c.bin"), &base).expect("c");

    let counts = app.scan_inbox().expect("scan inbox");
    assert_eq!(counts, import_counts(2, 1, 0));
    assert!(known(&app, "a.bin"));
    assert!(known(&app, "b.bin"));
    assert!(!inbox.join("a.bin").exists());
    assert!(!inbox.join("b.bin").exists());
    assert!(!inbox.join("c.bin").exists(), "vrai doublon supprimé");
    assert_eq!(
        std::fs::read(stored_path(&app, "b.bin")).expect("b"),
        different
    );
    assert_eq!(std::fs::read(stored_path(&app, "a.bin")).expect("a"), base);
}

#[test]
fn scan_inbox_is_idempotent_and_leaves_it_empty() {
    let harness = Harness::with_inbox();
    let inbox = harness.inbox_path();
    std::fs::write(inbox.join("stable.txt"), b"stable").expect("dépôt");

    let first = harness.app.scan_inbox().expect("premier scan");
    assert_eq!(first.imported, 1);
    let second = harness.app.scan_inbox().expect("second scan");
    assert_eq!(second, ImportCounts::default());
    assert_eq!(std::fs::read_dir(&inbox).expect("inbox").count(), 0);
}

#[test]
fn scan_inbox_removes_source_directories_left_empty() {
    let harness = Harness::with_inbox();
    let inbox = harness.inbox_path();
    let nested = inbox.join("photos/2026");
    std::fs::create_dir_all(&nested).expect("dossier");
    std::fs::write(nested.join("visible.txt"), b"visible").expect("visible");

    let counts = harness.app.scan_inbox().expect("scan inbox");
    assert_eq!(counts.imported, 1);
    assert!(!nested.exists(), "feuille vide supprimée");
    assert!(!inbox.join("photos").exists(), "parent vide supprimé");
    assert_eq!(std::fs::read_dir(&inbox).expect("inbox").count(), 0);
}

#[test]
fn scan_inbox_ignores_directory_swapped_for_a_symlink() {
    let (temp, app, fs, inbox) = scripted_setup();
    let nested = inbox.join("zzz-sub");
    std::fs::create_dir_all(&nested).expect("sous-dossier");
    std::fs::write(nested.join("victime.txt"), b"victime").expect("victime");
    let moved = inbox.join("zzz-sub-moved");
    let target = temp.path().join("cible");
    std::fs::create_dir_all(&target).expect("cible");
    std::fs::write(target.join("canary.txt"), b"canary").expect("canary");
    fs.swap_dir_on_reopen(&nested, &moved, &target);

    let counts = app.scan_inbox().expect("scan inbox");
    assert_eq!(counts, import_counts(0, 0, 0));
    assert!(target.join("canary.txt").exists(), "canari jamais supprimé");
    assert!(
        !stored_path(&app, "canary.txt").exists(),
        "canari jamais importé"
    );
    assert!(missing(&app, "canary.txt"));
    assert!(
        moved.join("victime.txt").exists(),
        "source réelle conservée"
    );
    assert!(files_under(&app.files_path().expect("files")).is_empty());
}

#[test]
fn scan_inbox_imports_real_subdir_and_removes_source() {
    let harness = Harness::with_inbox();
    let inbox = harness.inbox_path();
    let nested = inbox.join("photos/2026");
    std::fs::create_dir_all(&nested).expect("dossier");
    let source = nested.join("photo.txt");
    std::fs::write(&source, b"photo").expect("photo");

    let counts = harness.app.scan_inbox().expect("scan inbox");
    assert_eq!(counts, import_counts(1, 0, 0));
    assert_eq!(
        std::fs::read(stored_path(&harness.app, "photo.txt")).expect("photo"),
        b"photo"
    );
    assert!(!source.exists(), "source retirée");
    assert!(!nested.exists(), "dossier vidé retiré");
}

#[test]
fn scan_inbox_keeps_directory_holding_an_ignored_file() {
    let harness = Harness::with_inbox();
    let inbox = harness.inbox_path();
    let nested = inbox.join("photos/2026");
    std::fs::create_dir_all(&nested).expect("dossier");
    std::fs::write(nested.join("visible.txt"), b"visible").expect("visible");
    std::fs::write(nested.join("garde.part"), b"part").expect("partiel");

    let counts = harness.app.scan_inbox().expect("scan inbox");
    assert_eq!(counts.imported, 1);
    assert!(!nested.join("visible.txt").exists(), "visible importé");
    assert!(
        nested.join("garde.part").exists(),
        "fichier ignoré conservé"
    );
    assert!(nested.exists(), "dossier non vide conservé");
    assert!(inbox.join("photos").exists(), "parent non vide conservé");
}
