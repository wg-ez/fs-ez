use super::*;

#[test]
fn manual_scan_reports_duplicates_and_ignores_temporary_names() {
    let harness = Harness::with_files();
    harness.write("files/un.txt", b"un");
    harness.write("files/deux.txt", b"deux");
    harness.write("files/copie.txt", b"un");
    harness.write("files/.cache", b"cache");
    harness.write("files/fichier.part", b"partiel");

    let report = harness.app.scan_files().expect("scan");
    assert_eq!(report.counts(), import_counts(2, 1, 0));
    assert_eq!(report.imported.len(), 2);
    assert_eq!(report.duplicates.len(), 1);
    assert!(report.skipped.is_empty());
    assert!(harness.temp.path().join("files/copie.txt").is_file());
}

#[test]
fn scan_finds_a_file_already_stored_in_its_shard() {
    let harness = Harness::with_files();
    let files = harness.temp.path().join("files");
    let path = sharded_path(&files, OsStr::new("range.txt"));
    std::fs::create_dir_all(path.parent().expect("shard")).expect("shard");
    std::fs::write(&path, b"range").expect("fichier");

    let report = harness.app.scan_files().expect("scan");
    assert_eq!(report.counts(), import_counts(1, 0, 0));
    assert_eq!(report.imported[0].path, path.canonicalize().expect("canon"));
}

#[test]
fn explicit_scan_is_idempotent() {
    let harness = Harness::with_files();
    harness.write("files/stable.txt", b"stable");
    let first = harness.app.scan_files().expect("premier scan");
    assert_eq!(first.imported.len(), 1);
    let second = harness.app.scan_files().expect("second scan");
    assert!(second.imported.is_empty());
    assert!(second.duplicates.is_empty());
}

#[test]
fn scan_imports_sampled_collision_whose_middle_differs() {
    use fs_ez_fingerprint::Blake3Fingerprinter;

    let temp = tempfile::TempDir::new().expect("racine");
    let files = temp.path().join("files");
    std::fs::create_dir_all(&files).expect("files");
    let fingerprinter = Arc::new(Blake3Fingerprinter::new(1024, 256).expect("configuration"));
    let app = app_with_fingerprinter(&temp, fingerprinter);
    let base = vec![7u8; 4096];
    let mut different = base.clone();
    different[2048] = 8;
    std::fs::write(files.join("a.bin"), &base).expect("a");
    std::fs::write(files.join("b.bin"), &different).expect("b");

    let report = app.scan_files().expect("scan");
    assert_eq!(report.counts(), import_counts(2, 0, 0));
    assert_eq!(std::fs::read(files.join("a.bin")).expect("a"), base);
    assert_eq!(std::fs::read(files.join("b.bin")).expect("b"), different);
    let again = app.scan_files().expect("second scan");
    assert_eq!(again.counts(), import_counts(0, 0, 0));
}

#[test]
fn outcomes_expose_stable_statuses() {
    let harness = Harness::with_files();
    let path = harness.write("files/statut.txt", b"statut");
    assert_eq!(
        harness.app.import_path(&path).expect("import").status(),
        "imported"
    );
    assert_eq!(
        harness.app.import_path(&path).expect("ré-import").status(),
        "imported"
    );
    let other = harness.write("files/copie-statut.txt", b"statut");
    assert_eq!(
        harness.app.import_path(&other).expect("doublon").status(),
        "duplicate"
    );
}

#[test]
fn copy_elsewhere_is_duplicate_and_modification_updates_metadata() {
    let harness = Harness::new();
    let app = &harness.app;
    let first = write(&harness.temp, "first.txt", b"same");
    app.import_path(&first).expect("premier import");
    let copy = harness.temp.path().join("copy.txt");
    std::fs::copy(&first, &copy).expect("copie");

    let LibraryOutcome::Duplicate { existing } = app.import_path(&copy).expect("copie") else {
        panic!("doublon attendu");
    };
    assert_eq!(existing.path, std::fs::canonicalize(&first).expect("canon"));

    std::fs::write(&first, b"changed content").expect("modification");
    let LibraryOutcome::Indexed(file) = app.import_path(&first).expect("ré-import") else {
        panic!("même chemin attendu");
    };
    assert_eq!(file.size, 15);
}
