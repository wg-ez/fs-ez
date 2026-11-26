//! Tests d'import d'un upload DAV : assainissement et collisions.

use std::ffi::OsStr;
use std::path::PathBuf;
use std::time::SystemTime;

use fs_ez_app::{LibraryOutcome, sharded_path};

mod common;

use common::{Harness, files_under, temp_upload};

#[test]
fn upload_lands_in_files_and_queues_pending() {
    let harness = Harness::with_files();
    let temp = temp_upload(&harness, "upload-a.part", b"hello upload");
    let outcome = harness
        .app
        .import_upload(&temp, "note.txt", None)
        .expect("import");

    let LibraryOutcome::Indexed(file) = outcome else {
        panic!("import attendu");
    };
    let files = harness.temp.path().join("files");
    let destination = sharded_path(&files, OsStr::new("note.txt"));
    assert!(destination.is_file());
    assert_eq!(destination.file_name(), Some(OsStr::new("note.txt")));
    assert!(!files.join("note.txt").exists());
    assert!(!temp.exists());
    assert_eq!(file.path.file_name(), Some(OsStr::new("note.txt")));
    assert_eq!(file.path, destination.canonicalize().expect("canon"));
}

#[test]
fn upload_restores_client_mtime() {
    let harness = Harness::with_files();
    let temp = temp_upload(&harness, "upload-mtime.part", b"dated");
    let stamp = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("date");

    let outcome = harness
        .app
        .import_upload(&temp, "dated.txt", Some(stamp))
        .expect("import");
    let LibraryOutcome::Indexed(file) = outcome else {
        panic!("import attendu");
    };
    assert_eq!(file.mtime, stamp);

    let files = harness.temp.path().join("files");
    let destination = sharded_path(&files, OsStr::new("dated.txt"));
    assert_eq!(
        std::fs::metadata(&destination)
            .expect("meta")
            .modified()
            .expect("mtime"),
        SystemTime::from(stamp)
    );
}

#[test]
fn sanitized_names_never_escape_and_skip_leading_dots() {
    let harness = Harness::with_files();
    let temp = temp_upload(&harness, "upload-b.part", b"passwd");
    harness
        .app
        .import_upload(&temp, "../../etc/passwd", None)
        .expect("import");
    let files = harness.temp.path().join("files");
    assert!(sharded_path(&files, OsStr::new("passwd")).is_file());

    let hidden = temp_upload(&harness, "upload-c.part", b"hidden");
    harness
        .app
        .import_upload(&hidden, ".hidden", None)
        .expect("import");
    assert!(sharded_path(&files, OsStr::new("hidden")).is_file());
    assert!(!sharded_path(&files, OsStr::new(".hidden")).exists());
}

#[test]
fn same_name_with_different_content_gets_a_suffix() {
    let harness = Harness::with_files();
    let first = temp_upload(&harness, "upload-d.part", b"first");
    harness
        .app
        .import_upload(&first, "photo.png", None)
        .expect("import");
    let second = temp_upload(&harness, "upload-e.part", b"second");
    harness
        .app
        .import_upload(&second, "photo.png", None)
        .expect("import");

    let files = harness.temp.path().join("files");
    let first = sharded_path(&files, OsStr::new("photo.png"));
    let stored = files_under(&files);
    let names: Vec<String> = stored
        .iter()
        .map(|path| {
            path.file_name()
                .expect("nom")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(names.len(), 2, "{names:?}");
    assert!(names.contains(&"photo.png".to_string()));
    let suffixed = stored
        .iter()
        .find(|path| {
            let name = path.file_name().expect("nom").to_string_lossy();
            name.starts_with("photo-") && name.ends_with(".png")
        })
        .unwrap_or_else(|| panic!("suffixe absent: {names:?}"));
    assert!(first.is_file());
    assert!(suffixed.is_file());
    assert_ne!(
        std::fs::read(&first).expect("contenu"),
        std::fs::read(suffixed).expect("contenu")
    );
}

#[test]
fn duplicate_upload_is_removed_and_original_kept() {
    let harness = Harness::with_files();
    let original = harness.temp.path().join("original.txt");
    std::fs::write(&original, b"same bytes").expect("écriture");
    harness.app.import_path(&original).expect("import original");

    let temp = temp_upload(&harness, "upload-f.part", b"same bytes");
    let outcome = harness
        .app
        .import_upload(&temp, "copie.txt", None)
        .expect("upload");
    let LibraryOutcome::Duplicate { existing } = outcome else {
        panic!("doublon attendu");
    };
    assert_eq!(existing.path, original.canonicalize().expect("canon"));
    assert!(original.is_file());
    let files = harness.temp.path().join("files");
    assert!(!sharded_path(&files, OsStr::new("copie.txt")).exists());
    assert!(!temp.exists());
}

#[test]
fn only_used_shard_leaves_are_created() {
    let harness = Harness::with_files();
    let temp = temp_upload(&harness, "upload-g.part", b"lazy");
    harness
        .app
        .import_upload(&temp, "note.txt", None)
        .expect("import");

    let files = harness.temp.path().join("files");
    let first: Vec<_> = std::fs::read_dir(&files)
        .expect("niveau 1")
        .map(|entry| entry.expect("entrée").file_name())
        .collect();
    assert_eq!(first.len(), 1, "{first:?}");
    let second: Vec<_> = std::fs::read_dir(files.join(&first[0]))
        .expect("niveau 2")
        .map(|entry| entry.expect("entrée").file_name())
        .collect();
    assert_eq!(second.len(), 1, "{second:?}");
}

/// Fichier de bibliothèque posé directement dans son shard, sans passer par un upload.
fn managed_file(harness: &Harness, name: &str, content: &[u8]) -> PathBuf {
    let files = harness.temp.path().join("files");
    let path = sharded_path(&files, OsStr::new(name));
    std::fs::create_dir_all(path.parent().expect("shard")).expect("shard");
    std::fs::write(&path, content).expect("écriture");
    path
}

#[test]
fn clean_upload_reclaims_a_suffixed_duplicate_keeping_tags() {
    use fs_ez_core::TagColor;

    let harness = Harness::with_files();
    let suffixed = managed_file(&harness, "race-3fa9c1.txt", b"shared payload");
    let LibraryOutcome::Indexed(original) =
        harness.app.import_path(&suffixed).expect("index initial")
    else {
        panic!("ligne initiale attendue");
    };
    let tag = common::create_tag(&harness.app, "Course", TagColor::Blue);
    harness
        .app
        .store
        .apply_tag(original.id, tag, true, time::OffsetDateTime::now_utc())
        .expect("tag");

    let temp = temp_upload(&harness, "upload-race.part", b"shared payload");
    let outcome = harness
        .app
        .import_upload(&temp, "race.txt", None)
        .expect("upload");

    let LibraryOutcome::Indexed(file) = outcome else {
        panic!("le nom propre doit gagner: {outcome:?}");
    };
    let files = harness.temp.path().join("files");
    let clean = sharded_path(&files, OsStr::new("race.txt"))
        .canonicalize()
        .expect("canon");
    assert_eq!(file.id, original.id, "l'identifiant doit être conservé");
    assert_eq!(file.path, clean);
    assert_eq!(file.name, "race.txt");
    assert_eq!(harness.app.store.count_files().expect("compte"), 1);

    let tags = harness.app.store.tags_for_file(original.id).expect("tags");
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].tag.id, tag);

    assert!(
        !suffixed.exists(),
        "ancienne copie suffixée encore présente"
    );
    assert_eq!(files_under(&files), vec![clean]);
}

#[test]
fn duplicate_upload_after_clean_name_is_removed() {
    let harness = Harness::with_files();
    let files = harness.temp.path().join("files");
    let first = temp_upload(&harness, "upload-first.part", b"same bytes");
    harness
        .app
        .import_upload(&first, "order.txt", None)
        .expect("premier");
    let clean = sharded_path(&files, OsStr::new("order.txt"))
        .canonicalize()
        .expect("canon");

    let second = temp_upload(&harness, "upload-second.part", b"same bytes");
    let outcome = harness
        .app
        .import_upload(&second, "order.txt", None)
        .expect("second");
    let LibraryOutcome::Duplicate { existing } = outcome else {
        panic!("doublon attendu: {outcome:?}");
    };

    assert_eq!(existing.path, clean);
    assert_eq!(harness.app.store.count_files().expect("compte"), 1);
    assert_eq!(files_under(&files), vec![clean]);
    assert!(!second.exists(), "temporaire restant");
}

#[test]
fn sampled_edge_collision_upload_is_kept_with_complete_fingerprint() {
    use fs_ez_fingerprint::Blake3Fingerprinter;
    use std::sync::Arc;

    let temp = tempfile::TempDir::new().expect("racine");
    std::fs::create_dir_all(temp.path().join("files")).expect("files");
    let fingerprinter = Arc::new(Blake3Fingerprinter::new(1024, 256).expect("configuration"));
    let app = common::app_with_fingerprinter(&temp, fingerprinter);
    let tmp = app.upload_tmp_dir();
    std::fs::create_dir_all(&tmp).expect("tmp");
    let base = vec![7u8; 4096];
    let mut different = base.clone();
    different[2048] = 8;

    let first = tmp.join("first.part");
    std::fs::write(&first, &base).expect("first");
    let LibraryOutcome::Indexed(_) = app.import_upload(&first, "a.bin", None).expect("a") else {
        panic!("premier import attendu");
    };
    let second = tmp.join("second.part");
    std::fs::write(&second, &different).expect("second");
    let outcome = app.import_upload(&second, "b.bin", None).expect("b");
    let LibraryOutcome::Indexed(_) = outcome else {
        panic!("collision de bords : import attendu: {outcome:?}");
    };

    assert_eq!(app.store.count_files().expect("compte"), 2);
    let files = temp.path().join("files");
    assert_eq!(
        std::fs::read(sharded_path(&files, OsStr::new("a.bin"))).expect("a"),
        base
    );
    assert_eq!(
        std::fs::read(sharded_path(&files, OsStr::new("b.bin"))).expect("b"),
        different
    );
}

#[test]
fn same_name_with_other_content_keeps_both_rows() {
    let harness = Harness::with_files();
    let files = harness.temp.path().join("files");
    let first = temp_upload(&harness, "upload-one.part", b"alpha");
    harness
        .app
        .import_upload(&first, "partage.txt", None)
        .expect("premier");
    let second = temp_upload(&harness, "upload-two.part", b"beta");
    let LibraryOutcome::Indexed(other) = harness
        .app
        .import_upload(&second, "partage.txt", None)
        .expect("second")
    else {
        panic!("contenu distinct : deux imports attendus");
    };

    assert_ne!(
        other.path.file_name(),
        Some(OsStr::new("partage.txt")),
        "le second porteur doit être suffixé"
    );
    assert_eq!(harness.app.store.count_files().expect("compte"), 2);
    assert_eq!(files_under(&files).len(), 2);
    assert_eq!(
        std::fs::read(sharded_path(&files, OsStr::new("partage.txt"))).expect("contenu"),
        b"alpha"
    );
}

#[test]
fn duplicate_outside_the_managed_shard_is_never_swapped() {
    let harness = Harness::with_files();
    let files = harness.temp.path().join("files");
    let manual = harness.temp.path().join("manual").join("race-3fa9c1.txt");
    std::fs::create_dir_all(manual.parent().expect("dossier")).expect("dossier");
    std::fs::write(&manual, b"shared payload").expect("écriture");
    harness.app.import_path(&manual).expect("index manuel");

    let temp = temp_upload(&harness, "upload-outside.part", b"shared payload");
    let outcome = harness
        .app
        .import_upload(&temp, "race.txt", None)
        .expect("upload");
    let LibraryOutcome::Duplicate { existing } = outcome else {
        panic!("aucun échange hors shard géré: {outcome:?}");
    };

    assert_eq!(existing.path, manual.canonicalize().expect("canon"));
    assert!(manual.is_file(), "le fichier hors shard est conservé");
    assert!(!sharded_path(&files, OsStr::new("race.txt")).exists());
    assert_eq!(harness.app.store.count_files().expect("compte"), 1);
}
