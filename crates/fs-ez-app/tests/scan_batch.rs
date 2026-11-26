//! Rescan de la bibliothèque par lots : ordre, stabilité et replis unitaires.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, UNIX_EPOCH};

use fs_ez_app::ImportCounts;
use fs_ez_core::{FileKind, Fingerprint};
use fs_ez_fingerprint::{Blake3Fingerprinter, FingerprintError, Fingerprinter};
use tempfile::TempDir;

mod common;

use common::{Harness, app_with_fingerprinter, app_with_trigger_sql, modified, set_modified};

fn files_dir(temp: &TempDir) -> PathBuf {
    let files = temp.path().join("files");
    std::fs::create_dir_all(&files).expect("files");
    files
}

/// Compte les empreintes, c'est-à-dire les fichiers réellement rouverts.
struct CountingFingerprinter {
    inner: Blake3Fingerprinter,
    opened: AtomicUsize,
}

impl CountingFingerprinter {
    fn new() -> Self {
        Self {
            inner: Blake3Fingerprinter::default(),
            opened: AtomicUsize::new(0),
        }
    }
}

impl Fingerprinter for CountingFingerprinter {
    fn fingerprint(&self, path: &Path, kind: FileKind) -> Result<Fingerprint, FingerprintError> {
        self.opened.fetch_add(1, Ordering::SeqCst);
        self.inner.fingerprint(path, kind)
    }

    fn fingerprint_complete(&self, path: &Path) -> Result<Fingerprint, FingerprintError> {
        self.inner.fingerprint_complete(path)
    }

    fn full_hash_threshold(&self) -> u64 {
        self.inner.full_hash_threshold()
    }
}

/// Réécrit la cible après le calcul de son empreinte, une seule fois.
struct MutatingFingerprinter {
    inner: Blake3Fingerprinter,
    target: PathBuf,
    bytes: Vec<u8>,
    mutated: AtomicBool,
}

impl Fingerprinter for MutatingFingerprinter {
    fn fingerprint(&self, path: &Path, kind: FileKind) -> Result<Fingerprint, FingerprintError> {
        let fingerprint = self.inner.fingerprint(path, kind)?;
        if path == self.target && !self.mutated.swap(true, Ordering::SeqCst) {
            std::fs::write(&self.target, &self.bytes).expect("mutation pendant le hachage");
        }
        Ok(fingerprint)
    }

    fn fingerprint_complete(&self, path: &Path) -> Result<Fingerprint, FingerprintError> {
        self.inner.fingerprint_complete(path)
    }

    fn full_hash_threshold(&self) -> u64 {
        self.inner.full_hash_threshold()
    }
}

#[test]
fn rescan_updates_every_file_past_one_batch_in_walk_order() {
    let harness = Harness::with_files();
    let files = harness.temp.path().join("files");
    let mut names = Vec::new();
    for index in 0..520 {
        let name = format!("lot-{index:03}.txt");
        harness.write(
            &format!("files/{name}"),
            format!("initial {index:03}").as_bytes(),
        );
        names.push(name);
    }
    let first = harness.app.scan_files().expect("import initial");
    assert_eq!(first.counts().imported, 520);

    let stamp = UNIX_EPOCH + Duration::from_secs(1_700_100_000);
    for (index, name) in names.iter().enumerate() {
        let path = files.join(name);
        std::fs::write(&path, format!("modifié {index:03} plus long").as_bytes())
            .expect("écriture");
        set_modified(&path, stamp);
    }

    let report = harness.app.scan_files().expect("rescan");
    assert_eq!(
        report.counts(),
        ImportCounts {
            imported: 520,
            duplicates: 0,
            skipped: 0,
        }
    );
    let order: Vec<String> = report
        .imported
        .iter()
        .map(|record| record.name.clone())
        .collect();
    let mut expected = names.clone();
    expected.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    assert_eq!(order, expected, "ordre du parcours conservé");

    let second = harness.app.scan_files().expect("second rescan");
    assert_eq!(second.counts(), ImportCounts::default());
}

#[test]
fn unchanged_files_are_never_reopened_on_the_next_scan() {
    let temp = TempDir::new().expect("racine");
    let files = files_dir(&temp);
    for index in 0..520 {
        std::fs::write(
            files.join(format!("stable-{index:03}.txt")),
            format!("contenu stable {index:03}").as_bytes(),
        )
        .expect("écriture");
    }
    let fingerprinter = Arc::new(CountingFingerprinter::new());
    let app = app_with_fingerprinter(&temp, fingerprinter.clone());

    let first = app.scan_files().expect("import initial");
    assert_eq!(first.counts().imported, 520);
    assert_eq!(fingerprinter.opened.load(Ordering::SeqCst), 520);

    let second = app.scan_files().expect("rescan sans changement");
    assert_eq!(second.counts(), ImportCounts::default());
    assert_eq!(
        fingerprinter.opened.load(Ordering::SeqCst),
        520,
        "aucun fichier inchangé rouvert"
    );
}

#[test]
fn duplicate_on_another_path_is_detected_inside_one_batch() {
    let harness = Harness::with_files();
    let first = harness.write("files/a.txt", b"identique");
    harness.write("files/b.txt", b"identique");

    let report = harness.app.scan_files().expect("scan");

    assert_eq!(
        report.counts(),
        ImportCounts {
            imported: 1,
            duplicates: 1,
            skipped: 0,
        }
    );
    assert_eq!(
        report.duplicates[0].0,
        harness.temp.path().join("files/b.txt")
    );
    assert_eq!(
        report.duplicates[0].1.path,
        first.canonicalize().expect("canon")
    );
}

#[test]
fn file_changed_during_hash_is_not_indexed() {
    let temp = TempDir::new().expect("racine");
    let files = files_dir(&temp);
    let target = files.join("mouvant.txt");
    std::fs::write(&target, b"initial").expect("dépôt");
    let fingerprinter = Arc::new(MutatingFingerprinter {
        inner: Blake3Fingerprinter::default(),
        target: target.clone(),
        bytes: b"contenu change pendant le hachage".to_vec(),
        mutated: AtomicBool::new(false),
    });
    let app = app_with_fingerprinter(&temp, fingerprinter);

    let report = app.scan_files().expect("scan");
    assert_eq!(
        report.counts(),
        ImportCounts {
            imported: 0,
            duplicates: 0,
            skipped: 1,
        }
    );
    assert!(app.find_by_name("mouvant.txt").expect("base").is_none());

    let second = app.scan_files().expect("second scan");
    assert_eq!(second.counts().imported, 1);
    assert_eq!(
        std::fs::read(&target).expect("contenu"),
        b"contenu change pendant le hachage"
    );
}

#[test]
fn failed_update_keeps_the_other_files_of_the_batch() {
    let temp = TempDir::new().expect("racine");
    let files = files_dir(&temp);
    std::fs::write(files.join("boom.txt"), b"boom").expect("boom");
    std::fs::write(files.join("ok.txt"), b"ok").expect("ok");
    // Trigger posé avant l'import : il ne bloque que les mises à jour, pas les insertions.
    let app = app_with_trigger_sql(
        &temp,
        "CREATE TRIGGER scan_update_boom BEFORE UPDATE ON file WHEN OLD.name = 'boom.txt'
         BEGIN SELECT RAISE(ABORT, 'boom'); END;",
    );

    let report = app.scan_files().expect("scan");
    assert_eq!(
        report.counts(),
        ImportCounts {
            imported: 2,
            duplicates: 0,
            skipped: 0,
        }
    );
    let stamp = UNIX_EPOCH + Duration::from_secs(1_700_200_000);
    for name in ["boom.txt", "ok.txt"] {
        let path = files.join(name);
        std::fs::write(&path, format!("{name} modifié et plus long").as_bytes()).expect("écriture");
        set_modified(&path, stamp);
    }

    let report = app.scan_files().expect("rescan");
    assert_eq!(
        report.counts(),
        ImportCounts {
            imported: 1,
            duplicates: 0,
            skipped: 1,
        }
    );
    assert_eq!(report.skipped[0].0, files.join("boom.txt"));
    let updated = app.find_by_name("ok.txt").expect("base").expect("fichier");
    assert_eq!(updated.size, "ok.txt modifié et plus long".len() as u64);
}

#[test]
fn failed_insert_on_a_new_file_is_skipped_alone() {
    let temp = TempDir::new().expect("racine");
    let files = files_dir(&temp);
    std::fs::write(files.join("boom.txt"), b"boom").expect("boom");
    std::fs::write(files.join("ok.txt"), b"ok").expect("ok");
    let app = app_with_trigger_sql(
        &temp,
        "CREATE TRIGGER scan_insert_boom BEFORE INSERT ON file WHEN NEW.name = 'boom.txt'
         BEGIN SELECT RAISE(ABORT, 'boom'); END;",
    );

    let report = app.scan_files().expect("scan");
    assert_eq!(
        report.counts(),
        ImportCounts {
            imported: 1,
            duplicates: 0,
            skipped: 1,
        }
    );
    assert!(app.find_by_name("ok.txt").expect("base").is_some());
    assert!(app.find_by_name("boom.txt").expect("base").is_none());
}

#[test]
fn scan_keeps_unchanged_files_closed() {
    let harness = Harness::with_files();
    let path = harness.write("files/stable.txt", b"aaaa");
    harness.app.scan_files().expect("premier scan");
    let before = harness
        .app
        .find_by_path(&path)
        .expect("lecture")
        .expect("connu");
    let fingerprint = before.fingerprint.clone();
    let stamp = modified(&path);
    std::fs::write(&path, b"bbbb").expect("contenu");
    set_modified(&path, stamp);

    let report = harness.app.scan_files().expect("second scan");

    assert_eq!(report.counts(), ImportCounts::default());
    let after = harness
        .app
        .find_by_path(&path)
        .expect("lecture")
        .expect("connu");
    assert_eq!(after.fingerprint, fingerprint);
}

#[test]
fn scan_crosses_path_batches_and_stays_idempotent() {
    let harness = Harness::with_files();
    for index in 0..520 {
        harness.write(
            &format!("files/lot-{index:03}.txt"),
            format!("contenu {index:03}").as_bytes(),
        );
    }

    let first = harness.app.scan_files().expect("premier scan");
    assert_eq!(first.counts().imported, 520);
    assert_eq!(first.counts().duplicates, 0);
    let second = harness.app.scan_files().expect("second scan");
    assert_eq!(second.counts(), ImportCounts::default());
}

#[test]
fn scan_files_ignores_hidden_temp_and_directories() {
    let harness = Harness::new();
    let app = &harness.app;
    let files = harness.temp.path().join("files");
    std::fs::create_dir(&files).expect("files");
    std::fs::write(files.join("a.txt"), b"a").expect("a");
    std::fs::write(files.join("b.txt"), b"b").expect("b");
    std::fs::write(files.join(".hidden"), b"h").expect("hidden");
    std::fs::write(files.join("x.part"), b"p").expect("part");
    let nested = files.join("sub/deep");
    std::fs::create_dir_all(&nested).expect("dossier");
    std::fs::write(nested.join("c.txt"), b"c").expect("c");
    std::fs::write(nested.join(".cache"), b"h").expect("cache");

    assert_eq!(
        app.files_path().expect("chemin"),
        std::fs::canonicalize(&files).expect("canon")
    );
    let report = app.scan_files().expect("scan");
    assert_eq!(report.imported.len(), 3);
    assert!(report.duplicates.is_empty());
    assert!(report.skipped.is_empty());

    let mut names: Vec<String> = report
        .imported
        .iter()
        .map(|file| file.name.clone())
        .collect();
    names.sort();
    assert_eq!(names, vec!["a.txt", "b.txt", "c.txt"]);
}
