use super::*;

#[test]
fn scan_inbox_links_same_volume_without_copy_or_temp() {
    let (temp, app, fs, inbox) = scripted_setup();
    let source = inbox.join("lien.txt");
    std::fs::write(&source, b"lien direct").expect("dépôt");
    let stamp = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    set_modified(&source, stamp);
    let source_inode = std::fs::metadata(&source).expect("meta source").ino();

    let counts = app.scan_inbox().expect("scan inbox");
    assert_eq!(counts, import_counts(1, 0, 0));
    let stored = stored_path(&app, "lien.txt");
    assert_eq!(std::fs::read(&stored).expect("fichier"), b"lien direct");
    assert_eq!(
        std::fs::metadata(&stored).expect("meta").ino(),
        source_inode,
        "lien matériel attendu, pas une copie"
    );
    assert_eq!(modified(&stored), stamp);
    assert!(!source.exists(), "source consommée");
    assert_eq!(fs.links.load(Ordering::SeqCst), 1);
    assert_eq!(fs.copies.load(Ordering::SeqCst), 0);
    assert_eq!(fs.copy_file_calls.load(Ordering::SeqCst), 0);
    assert!(
        !temp.path().join("config/tmp").exists(),
        "aucun temporaire créé en mode lien direct"
    );
}

#[test]
fn scan_inbox_duplicate_places_nothing_before_indexing() {
    let (temp, app, fs, inbox) = scripted_setup();
    let original = temp.path().join("files").join("original.txt");
    std::fs::write(&original, b"identique").expect("original");
    app.import_path(&original).expect("import original");
    let source = inbox.join("copie.txt");
    std::fs::write(&source, b"identique").expect("copie");

    let counts = app.scan_inbox().expect("scan inbox");
    assert_eq!(counts, import_counts(0, 1, 0));
    assert!(!source.exists(), "doublon vérifié supprimé");
    assert_eq!(std::fs::read(&original).expect("original"), b"identique");
    assert!(!stored_path(&app, "copie.txt").exists());
    assert_eq!(fs.links.load(Ordering::SeqCst), 0);
    assert_eq!(fs.copies.load(Ordering::SeqCst), 0);
    assert_eq!(fs.copy_file_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn scan_inbox_cross_device_copies_directly_and_restores_mtime() {
    let (_temp, app, fs, inbox) = scripted_setup();
    fs.force_cross_device();
    let source = inbox.join("autre-volume.txt");
    std::fs::write(&source, b"copie directe").expect("dépôt");
    let stamp = UNIX_EPOCH + Duration::from_secs(1_690_000_000);
    set_modified(&source, stamp);

    let counts = app.scan_inbox().expect("scan inbox");
    assert_eq!(counts, import_counts(1, 0, 0));
    let stored = stored_path(&app, "autre-volume.txt");
    assert_eq!(std::fs::read(&stored).expect("fichier"), b"copie directe");
    assert_eq!(modified(&stored), stamp);
    assert!(!source.exists());
    assert_eq!(fs.links.load(Ordering::SeqCst), 1);
    assert_eq!(fs.copies.load(Ordering::SeqCst), 1);
    assert_eq!(fs.copy_file_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn scan_inbox_index_failure_removes_destination_and_keeps_source() {
    let (_temp, app, fs, inbox) = scripted_setup();
    fs.drop_after_link();
    let source = inbox.join("fragile.txt");
    std::fs::write(&source, b"fragile").expect("dépôt");

    let counts = app.scan_inbox().expect("scan inbox");
    assert_eq!(counts, import_counts(0, 0, 1));
    assert!(source.exists(), "source intacte après échec");
    let stored = stored_path(&app, "fragile.txt");
    assert!(!stored.exists(), "destination retirée après échec");
    assert!(missing(&app, "fragile.txt"));
}

#[test]
fn scan_inbox_source_changed_during_copy_is_skipped() {
    let (_temp, app, fs, inbox) = scripted_setup();
    fs.force_cross_device();
    fs.mutate_source_before_copy(b"mute avant la copie cross-device");
    let source = inbox.join("mouvant.txt");
    std::fs::write(&source, b"initial").expect("dépôt");

    let counts = app.scan_inbox().expect("scan inbox");
    assert_eq!(counts, import_counts(0, 0, 1));
    assert_eq!(
        std::fs::read(&source).expect("source"),
        b"mute avant la copie cross-device"
    );
    let stored = stored_path(&app, "mouvant.txt");
    assert!(!stored.exists(), "copie potentiellement déchirée retirée");
    assert!(missing(&app, "mouvant.txt"));
}

#[test]
fn scan_inbox_source_changed_after_link_reindexes_destination() {
    use fs_ez_core::FileKind;
    use fs_ez_fingerprint::{Blake3Fingerprinter, Fingerprinter};

    let (_temp, app, fs, inbox) = scripted_setup();
    fs.mutate_source_after_link(b"contenu mute apres le lien");
    let source = inbox.join("mutable.txt");
    std::fs::write(&source, b"initial").expect("dépôt");

    let counts = app.scan_inbox().expect("scan inbox");
    assert_eq!(counts, import_counts(1, 0, 0));
    let stored = stored_path(&app, "mutable.txt");
    assert_eq!(
        std::fs::read(&stored).expect("destination"),
        b"contenu mute apres le lien"
    );
    assert!(!source.exists(), "source consommée après réindexation");
    let record = app
        .find_by_name("mutable.txt")
        .expect("base")
        .expect("fichier");
    let fingerprint = Blake3Fingerprinter::default()
        .fingerprint(&stored, FileKind::File)
        .expect("empreinte destination");
    assert_eq!(
        record.fingerprint,
        Some(fingerprint),
        "empreinte stockée recalculée sur la destination"
    );
}

#[test]
fn scan_inbox_links_pinned_source_after_directory_swap() {
    let (temp, app, fs, inbox) = scripted_setup();
    let nested = inbox.join("zzz-sub");
    std::fs::create_dir_all(&nested).expect("sous-dossier");
    std::fs::write(nested.join("piece.txt"), b"interne").expect("pièce");
    let moved = inbox.join("zzz-sub-moved");
    let target = temp.path().join("cible");
    std::fs::create_dir_all(&target).expect("cible");
    std::fs::write(target.join("piece.txt"), b"canary").expect("canari");
    fs.swap_dir_before_link(&nested, &moved, &target);

    let counts = app.scan_inbox().expect("scan inbox");
    assert_eq!(counts, import_counts(1, 0, 0));
    let stored = stored_path(&app, "piece.txt");
    assert_eq!(
        std::fs::read(&stored).expect("destination"),
        b"interne",
        "lien matériel depuis le dossier épinglé, jamais depuis la cible"
    );
    assert_eq!(
        std::fs::read(target.join("piece.txt")).expect("canari"),
        b"canary",
        "cible hors inbox jamais lue ni supprimée"
    );
    assert!(
        moved.join("piece.txt").exists(),
        "source réelle conservée après refus du parent remplacé"
    );
}
