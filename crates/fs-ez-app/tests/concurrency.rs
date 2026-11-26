//! Courses de placement : revendication atomique de la destination.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::Duration;

use fs_ez_app::{App, AppError, LibraryOutcome, sharded_path};
use fs_ez_core::{FileId, FileKind};
use fs_ez_fingerprint::{Blake3Fingerprinter, Fingerprinter};
use fs_ez_fs::{DirEntryInfo, FileMeta, Fs, FsError, SystemFs};
use tempfile::TempDir;
use time::OffsetDateTime;

use common::{app_with_fs, files_under, write};

mod common;

/// Bloque le premier passage de `link_noreplace`, les deux appels voient la même cible libre.
struct Gate {
    barrier: Barrier,
    arrivals: AtomicUsize,
    parties: usize,
}

impl Gate {
    fn new(parties: usize) -> Self {
        Self {
            barrier: Barrier::new(parties),
            arrivals: AtomicUsize::new(0),
            parties,
        }
    }

    fn pass(&self) {
        if self.arrivals.fetch_add(1, Ordering::SeqCst) < self.parties {
            self.barrier.wait();
        }
    }
}

struct BarrierFs {
    inner: SystemFs,
    gate: Gate,
}

impl BarrierFs {
    fn new(parties: usize) -> Self {
        Self {
            inner: SystemFs,
            gate: Gate::new(parties),
        }
    }
}

impl Fs for BarrierFs {
    fn metadata(&self, path: &Path) -> Result<FileMeta, FsError> {
        self.inner.metadata(path)
    }

    fn canonicalize(&self, path: &Path) -> Result<PathBuf, FsError> {
        self.inner.canonicalize(path)
    }

    fn read_dir(&self, path: &Path) -> Result<Vec<DirEntryInfo>, FsError> {
        self.inner.read_dir(path)
    }

    fn read_dir_checked(
        &self,
        path: &Path,
        dev: u64,
        ino: u64,
    ) -> Result<Vec<DirEntryInfo>, FsError> {
        self.inner.read_dir_checked(path, dev, ino)
    }

    fn read_link(&self, path: &Path) -> Result<PathBuf, FsError> {
        self.inner.read_link(path)
    }

    fn read_file(&self, path: &Path) -> Result<Vec<u8>, FsError> {
        self.inner.read_file(path)
    }

    fn create_dir_all(&self, path: &Path) -> Result<(), FsError> {
        self.inner.create_dir_all(path)
    }

    fn remove_file(&self, path: &Path) -> Result<(), FsError> {
        self.inner.remove_file(path)
    }

    fn link_noreplace(&self, from: &Path, to: &Path) -> Result<(), FsError> {
        self.gate.pass();
        self.inner.link_noreplace(from, to)
    }

    fn copy_exclusive(&self, from: &Path, to: &Path) -> Result<(), FsError> {
        self.inner.copy_exclusive(from, to)
    }

    fn copy_file(&self, from: &Path, to: &Path) -> Result<(), FsError> {
        self.inner.copy_file(from, to)
    }

    fn set_mtime(&self, path: &Path, mtime: OffsetDateTime) -> Result<(), FsError> {
        self.inner.set_mtime(path, mtime)
    }

    fn writable(&self, path: &Path) -> bool {
        self.inner.writable(path)
    }
}

/// Bloque le lien du rename jusqu'à autorisation : fige la fenêtre lien → unlink.
struct HoldFs {
    inner: SystemFs,
    linked: Sender<()>,
    release: Mutex<Receiver<()>>,
}

impl Fs for HoldFs {
    fn metadata(&self, path: &Path) -> Result<FileMeta, FsError> {
        self.inner.metadata(path)
    }

    fn canonicalize(&self, path: &Path) -> Result<PathBuf, FsError> {
        self.inner.canonicalize(path)
    }

    fn read_dir(&self, path: &Path) -> Result<Vec<DirEntryInfo>, FsError> {
        self.inner.read_dir(path)
    }

    fn read_dir_checked(
        &self,
        path: &Path,
        dev: u64,
        ino: u64,
    ) -> Result<Vec<DirEntryInfo>, FsError> {
        self.inner.read_dir_checked(path, dev, ino)
    }

    fn read_link(&self, path: &Path) -> Result<PathBuf, FsError> {
        self.inner.read_link(path)
    }

    fn read_file(&self, path: &Path) -> Result<Vec<u8>, FsError> {
        self.inner.read_file(path)
    }

    fn create_dir_all(&self, path: &Path) -> Result<(), FsError> {
        self.inner.create_dir_all(path)
    }

    fn remove_file(&self, path: &Path) -> Result<(), FsError> {
        self.inner.remove_file(path)
    }

    fn link_noreplace(&self, from: &Path, to: &Path) -> Result<(), FsError> {
        self.inner.link_noreplace(from, to)?;
        let _ = self.linked.send(());
        if let Ok(release) = self.release.lock() {
            let _ = release.recv();
        }
        Ok(())
    }

    fn copy_exclusive(&self, from: &Path, to: &Path) -> Result<(), FsError> {
        self.inner.copy_exclusive(from, to)
    }

    fn copy_file(&self, from: &Path, to: &Path) -> Result<(), FsError> {
        self.inner.copy_file(from, to)
    }

    fn set_mtime(&self, path: &Path, mtime: OffsetDateTime) -> Result<(), FsError> {
        self.inner.set_mtime(path, mtime)
    }

    fn writable(&self, path: &Path) -> bool {
        self.inner.writable(path)
    }
}

fn barrier_app(temp: &TempDir, parties: usize) -> App {
    app_with_fs(temp, Arc::new(BarrierFs::new(parties)))
}

#[test]
fn concurrent_same_name_uploads_keep_one_consistent_file() {
    let temp = TempDir::new().expect("tempdir");
    std::fs::create_dir_all(temp.path().join("files")).expect("files");
    let app = barrier_app(&temp, 2);
    let tmp = app.upload_tmp_dir();
    std::fs::create_dir_all(&tmp).expect("tmp");
    let first = tmp.join("upload-first.part");
    let second = tmp.join("upload-second.part");
    std::fs::write(&first, b"shared payload").expect("écriture");
    std::fs::write(&second, b"shared payload").expect("écriture");

    let (left, right) = thread::scope(|scope| {
        let left = {
            let app = app.clone();
            let path = first.clone();
            scope.spawn(move || app.import_upload(&path, "race.txt", None))
        };
        let right = {
            let app = app.clone();
            let path = second.clone();
            scope.spawn(move || app.import_upload(&path, "race.txt", None))
        };
        (
            left.join().expect("thread gauche"),
            right.join().expect("thread droit"),
        )
    });

    let indexed = [&left, &right]
        .iter()
        .filter(|outcome| matches!(outcome, Ok(LibraryOutcome::Indexed(_))))
        .count();
    let duplicates = [&left, &right]
        .iter()
        .filter(|outcome| matches!(outcome, Ok(LibraryOutcome::Duplicate { .. })))
        .count();
    assert_eq!((indexed, duplicates), (1, 1), "{left:?} {right:?}");
    assert_eq!(app.store.count_files().expect("compte"), 1);

    let files = temp.path().join("files");
    let destination = sharded_path(&files, OsStr::new("race.txt"));
    let canonical = destination.canonicalize().expect("canon");
    assert!(canonical.is_file());
    assert_eq!(files_under(&files), vec![canonical.clone()]);
    assert!(!first.exists(), "temporaire restant");
    assert!(!second.exists(), "temporaire restant");

    let stored = app
        .store
        .find_file_by_path(&canonical)
        .expect("lecture")
        .expect("ligne");
    assert_eq!(stored.path, canonical, "la ligne doit porter le nom propre");
    let fingerprint = Blake3Fingerprinter::default()
        .fingerprint(&canonical, FileKind::File)
        .expect("empreinte");
    assert_eq!(stored.fingerprint, Some(fingerprint));
    assert_eq!(
        std::fs::read(&canonical).expect("contenu"),
        b"shared payload"
    );
}

#[test]
fn concurrent_renames_to_one_name_leave_both_contents_intact() {
    let temp = TempDir::new().expect("tempdir");
    let app = barrier_app(&temp, 2);
    let first = write(&temp, "first.txt", b"first");
    let second = write(&temp, "second.txt", b"second");
    let first_id = index(&app, &first);
    let second_id = index(&app, &second);

    let (left, right) = thread::scope(|scope| {
        let left = {
            let app = app.clone();
            scope.spawn(move || (first_id, app.rename_file(first_id, "target.txt")))
        };
        let right = {
            let app = app.clone();
            scope.spawn(move || (second_id, app.rename_file(second_id, "target.txt")))
        };
        (
            left.join().expect("thread gauche"),
            right.join().expect("thread droit"),
        )
    });

    let target = temp
        .path()
        .canonicalize()
        .expect("racine")
        .join("target.txt");
    let (winner, loser) = match (&left.1, &right.1) {
        (Ok(_), Err(_)) => (left, right),
        (Err(_), Ok(_)) => (right, left),
        _ => panic!("un seul renommage doit réussir: {left:?} {right:?}"),
    };
    assert!(matches!(loser.1, Err(AppError::Conflict(_))));
    assert!(target.is_file());

    let (winner_source, winner_bytes, loser_source, loser_bytes) = if winner.0 == first_id {
        (&first, b"first".as_slice(), &second, b"second".as_slice())
    } else {
        (&second, b"second".as_slice(), &first, b"first".as_slice())
    };
    assert_eq!(std::fs::read(&target).expect("contenu"), winner_bytes);
    assert!(!winner_source.exists());
    assert_eq!(std::fs::read(loser_source).expect("contenu"), loser_bytes);

    assert_eq!(app.store.count_files().expect("compte"), 2);
    for id in [first_id, second_id] {
        let record = app.store.get_file(id).expect("lecture").expect("ligne");
        assert!(record.path.is_file(), "{:?}", record.path);
    }
}

fn index(app: &App, path: &Path) -> FileId {
    match app.import_path(path).expect("import") {
        LibraryOutcome::Indexed(file) => file.id,
        LibraryOutcome::Duplicate { existing } => existing.id,
    }
}

#[test]
fn concurrent_rename_and_replace_never_lose_the_new_bytes() {
    let temp = TempDir::new().expect("tempdir");
    let (linked_tx, linked_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let app = app_with_fs(
        &temp,
        Arc::new(HoldFs {
            inner: SystemFs,
            linked: linked_tx,
            release: Mutex::new(release_rx),
        }),
    );
    let source = write(&temp, "source.txt", b"OLD-BYTES");
    let id = index(&app, &source);
    let staging_dir = app.upload_tmp_dir();
    std::fs::create_dir_all(&staging_dir).expect("tmp");
    let staging = staging_dir.join("replacement.part");
    std::fs::write(&staging, b"NEW-REPLACEMENT-BYTES").expect("écriture");
    let replacement_len = std::fs::metadata(&staging).expect("métadonnées").len();

    let (rename, replace) = thread::scope(|scope| {
        let rename_app = app.clone();
        let rename = scope.spawn(move || rename_app.rename_file(id, "renamed.txt"));
        linked_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("lien créé");

        let replace_app = app.clone();
        let replace = scope.spawn(move || replace_app.replace_file_contents(id, &staging, None));
        // Sans sérialisation, le remplacement aboutit avant le unlink et ses octets sont perdus.
        thread::sleep(Duration::from_millis(200));
        release_tx.send(()).expect("libération");
        (
            rename.join().expect("thread rename"),
            replace.join().expect("thread replace"),
        )
    });

    rename.expect("rename");
    replace.expect("replace");
    let record = app.store.get_file(id).expect("lecture").expect("ligne");
    let target = record.path.clone();
    assert_eq!(
        std::fs::read(&target).expect("contenu"),
        b"NEW-REPLACEMENT-BYTES"
    );
    assert_eq!(record.size, replacement_len);
    assert!(!source.exists(), "ancienne source encore présente");
}
