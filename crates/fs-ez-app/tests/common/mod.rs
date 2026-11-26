//! Harnais partagé des tests d'intégration de `fs-ez-app`.
#![allow(dead_code)]

use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use fs_ez_app::{App, ImportCounts, sharded_path};
use fs_ez_core::{GroupId, TagColor, TagId};
use fs_ez_fingerprint::{Blake3Fingerprinter, Fingerprinter};
use fs_ez_fs::{DirEntryInfo, FileMeta, Fs, FsError, Root, SystemFs};
use fs_ez_sqlite::{SqliteStore, db};
use fs_ez_store::Store;
use tempfile::TempDir;
use time::OffsetDateTime;

/// Groupe de test créé à la demande, les tags n'existent plus hors groupe.
pub fn test_group(app: &App) -> GroupId {
    app.list_groups()
        .expect("groupes")
        .into_iter()
        .next()
        .map(|group| group.id)
        .unwrap_or_else(|| app.create_group("Tests", None).expect("groupe").id)
}

pub fn create_tag(app: &App, name: &str, color: TagColor) -> TagId {
    app.create_tag(test_group(app), name, color)
        .expect("tag")
        .id
}

pub struct Harness {
    pub app: App,
    pub store: Arc<SqliteStore>,
    pub temp: TempDir,
}

impl Harness {
    pub fn new() -> Self {
        let temp = TempDir::new().expect("tempdir");
        let store = memory_store();
        let app = app_with_store(&temp, store.clone());
        Self { app, store, temp }
    }

    pub fn with_files() -> Self {
        let harness = Self::new();
        std::fs::create_dir_all(harness.temp.path().join("files")).expect("files");
        harness
    }

    pub fn with_inbox() -> Self {
        let harness = Self::with_files();
        harness.inbox_path();
        harness
    }

    pub fn inbox_path(&self) -> PathBuf {
        let dir = self.temp.path().join("inbox");
        std::fs::create_dir_all(&dir).expect("inbox");
        dir
    }

    pub fn stored_path(&self, name: &str) -> PathBuf {
        stored_path(&self.app, name)
    }

    pub fn write(&self, name: &str, content: &[u8]) -> PathBuf {
        write(&self.temp, name, content)
    }
}

pub fn import_counts(imported: usize, duplicates: usize, skipped: usize) -> ImportCounts {
    ImportCounts {
        imported,
        duplicates,
        skipped,
    }
}

pub fn stored_path(app: &App, name: &str) -> PathBuf {
    sharded_path(&app.files_path().expect("files"), OsStr::new(name))
}

pub fn temp_upload(harness: &Harness, name: &str, content: &[u8]) -> PathBuf {
    let dir = harness.app.upload_tmp_dir();
    std::fs::create_dir_all(&dir).expect("tmp");
    let path = dir.join(name);
    std::fs::write(&path, content).expect("écriture");
    path
}

pub fn memory_store() -> Arc<SqliteStore> {
    Arc::new(SqliteStore::from_connection(
        db::open_in_memory().expect("base mémoire"),
    ))
}

pub fn test_app(temp: &TempDir) -> App {
    app_with_store(temp, memory_store())
}

pub fn write(temp: &TempDir, name: &str, content: &[u8]) -> PathBuf {
    let path = temp.path().join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("dossier");
    }
    std::fs::write(&path, content).expect("écriture");
    path
}

pub fn set_modified(path: &Path, stamp: SystemTime) {
    let file = std::fs::File::options()
        .write(true)
        .open(path)
        .expect("ouverture");
    file.set_times(std::fs::FileTimes::new().set_modified(stamp))
        .expect("mtime");
}

pub fn modified(path: &Path) -> SystemTime {
    std::fs::metadata(path)
        .expect("meta")
        .modified()
        .expect("mtime")
}

pub fn files_under(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(dir).expect("read_dir") {
            let path = entry.expect("entrée").path();
            if path.is_dir() {
                stack.push(path);
            } else {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

pub fn root_of(temp: &TempDir) -> Root {
    Root::discover(Some(temp.path())).expect("root")
}

pub fn app_with_store(temp: &TempDir, store: Arc<SqliteStore>) -> App {
    app_with_store_at_root(store, root_of(temp))
}

pub fn app_with_store_at_root(store: Arc<SqliteStore>, root: Root) -> App {
    app_with_parts(
        store,
        Arc::new(SystemFs),
        Arc::new(Blake3Fingerprinter::default()),
        root,
    )
}

pub fn app_with_fingerprinter(temp: &TempDir, fingerprinter: Arc<dyn Fingerprinter>) -> App {
    app_with_parts(
        memory_store(),
        Arc::new(SystemFs),
        fingerprinter,
        root_of(temp),
    )
}

pub fn app_with_trigger_sql(temp: &TempDir, sql: &str) -> App {
    let conn = db::open_in_memory().expect("base mémoire");
    conn.execute_batch(sql).expect("trigger");
    app_with_store(temp, Arc::new(SqliteStore::from_connection(conn)))
}

pub fn app_with_fs(temp: &TempDir, fs: Arc<dyn Fs>) -> App {
    app_with_parts(
        memory_store(),
        fs,
        Arc::new(Blake3Fingerprinter::default()),
        root_of(temp),
    )
}

fn app_with_parts(
    store: Arc<SqliteStore>,
    fs: Arc<dyn Fs>,
    fingerprinter: Arc<dyn Fingerprinter>,
    root: Root,
) -> App {
    App::new(store as Arc<dyn Store>, fs, fingerprinter, root)
}

/// Remplacement d'un dossier listé par un lien, joué à sa réouverture.
struct SwapOnReopen {
    victim: PathBuf,
    moved: PathBuf,
    target: PathBuf,
}

/// Fs réel instrumenté : compte les placements et force cross-device ou lien perdu.
pub struct ScriptedFs {
    inner: SystemFs,
    pub links: AtomicUsize,
    pub copies: AtomicUsize,
    pub copy_file_calls: AtomicUsize,
    cross_device: AtomicBool,
    drop_after_link: AtomicBool,
    mutate_after_link: Mutex<Option<Vec<u8>>>,
    mutate_before_copy: Mutex<Option<Vec<u8>>>,
    fail_remove: Mutex<HashSet<String>>,
    swap_on_reopen: Mutex<Option<SwapOnReopen>>,
    swap_before_link: Mutex<Option<SwapOnReopen>>,
}

impl ScriptedFs {
    pub fn new() -> Self {
        Self {
            inner: SystemFs,
            links: AtomicUsize::new(0),
            copies: AtomicUsize::new(0),
            copy_file_calls: AtomicUsize::new(0),
            cross_device: AtomicBool::new(false),
            drop_after_link: AtomicBool::new(false),
            mutate_after_link: Mutex::new(None),
            mutate_before_copy: Mutex::new(None),
            fail_remove: Mutex::new(HashSet::new()),
            swap_on_reopen: Mutex::new(None),
            swap_before_link: Mutex::new(None),
        }
    }

    /// Simule un `files/` sur un autre volume : le lien échoue, la copie prend le relais.
    pub fn force_cross_device(&self) {
        self.cross_device.store(true, Ordering::SeqCst);
    }

    /// Supprime la destination juste après le lien : l'indexation échoue ensuite.
    pub fn drop_after_link(&self) {
        self.drop_after_link.store(true, Ordering::SeqCst);
    }

    /// Réécrit la source après le lien : la destination partage l'inode et change aussi.
    pub fn mutate_source_after_link(&self, bytes: &[u8]) {
        if let Ok(mut slot) = self.mutate_after_link.lock() {
            *slot = Some(bytes.to_vec());
        }
    }

    /// Réécrit la source entre le hash et la copie cross-device.
    pub fn mutate_source_before_copy(&self, bytes: &[u8]) {
        if let Ok(mut slot) = self.mutate_before_copy.lock() {
            *slot = Some(bytes.to_vec());
        }
    }

    /// Force l'échec d'un `unlink` pour ce nom de fichier, les autres passent.
    pub fn fail_remove_named(&self, name: &str) {
        if let Ok(mut names) = self.fail_remove.lock() {
            names.insert(name.to_string());
        }
    }

    /// Remplace un dossier listé par un lien vers `target` au moment de sa réouverture.
    pub fn swap_dir_on_reopen(&self, victim: &Path, moved: &Path, target: &Path) {
        if let Ok(mut slot) = self.swap_on_reopen.lock() {
            *slot = Some(SwapOnReopen {
                victim: victim.to_path_buf(),
                moved: moved.to_path_buf(),
                target: target.to_path_buf(),
            });
        }
    }

    /// Remplace un dossier par un lien vers `target` juste avant le lien matériel de placement.
    pub fn swap_dir_before_link(&self, victim: &Path, moved: &Path, target: &Path) {
        if let Ok(mut slot) = self.swap_before_link.lock() {
            *slot = Some(SwapOnReopen {
                victim: victim.to_path_buf(),
                moved: moved.to_path_buf(),
                target: target.to_path_buf(),
            });
        }
    }

    fn take_swap(slot: &Mutex<Option<SwapOnReopen>>) -> Option<SwapOnReopen> {
        slot.lock().ok().and_then(|mut slot| slot.take())
    }

    fn perform_swap(spec: SwapOnReopen) {
        std::fs::rename(&spec.victim, &spec.moved).expect("rename attaquant");
        std::os::unix::fs::symlink(&spec.target, &spec.victim).expect("symlink attaquant");
    }

    fn take(slot: &Mutex<Option<Vec<u8>>>) -> Option<Vec<u8>> {
        slot.lock().ok().and_then(|mut slot| slot.take())
    }

    fn maybe_swap(&self, path: &Path) {
        let swap = self.swap_on_reopen.lock().ok().and_then(|mut slot| {
            if slot.as_ref().is_some_and(|spec| spec.victim == path) {
                slot.take()
            } else {
                None
            }
        });
        if let Some(spec) = swap {
            Self::perform_swap(spec);
        }
    }
}

impl Fs for ScriptedFs {
    fn metadata(&self, path: &Path) -> Result<FileMeta, FsError> {
        self.inner.metadata(path)
    }

    fn canonicalize(&self, path: &Path) -> Result<PathBuf, FsError> {
        self.inner.canonicalize(path)
    }

    fn read_dir(&self, path: &Path) -> Result<Vec<DirEntryInfo>, FsError> {
        self.maybe_swap(path);
        self.inner.read_dir(path)
    }

    fn read_dir_checked(
        &self,
        path: &Path,
        dev: u64,
        ino: u64,
    ) -> Result<Vec<DirEntryInfo>, FsError> {
        self.maybe_swap(path);
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
        if let Some(name) = path.file_name().and_then(|name| name.to_str())
            && self
                .fail_remove
                .lock()
                .map(|names| names.contains(name))
                .unwrap_or(false)
        {
            return Err(FsError::Io(format!("forced unlink failure: {name}")));
        }
        self.inner.remove_file(path)
    }

    fn link_noreplace(&self, from: &Path, to: &Path) -> Result<(), FsError> {
        // Injection : le dossier source est remplacé par un lien juste avant le lien matériel.
        if let Some(spec) = Self::take_swap(&self.swap_before_link) {
            Self::perform_swap(spec);
        }
        self.links.fetch_add(1, Ordering::SeqCst);
        if self.cross_device.load(Ordering::SeqCst) {
            return Err(FsError::CrossDevice("forced".to_string()));
        }
        self.inner.link_noreplace(from, to)?;
        if self.drop_after_link.load(Ordering::SeqCst) {
            let _ = std::fs::remove_file(to);
        }
        if let Some(bytes) = Self::take(&self.mutate_after_link) {
            std::fs::write(from, bytes).expect("mutation après lien");
        }
        Ok(())
    }

    fn copy_exclusive(&self, from: &Path, to: &Path) -> Result<(), FsError> {
        self.copies.fetch_add(1, Ordering::SeqCst);
        if let Some(bytes) = Self::take(&self.mutate_before_copy) {
            std::fs::write(from, bytes).expect("mutation avant copie");
        }
        self.inner.copy_exclusive(from, to)
    }

    fn copy_file(&self, from: &Path, to: &Path) -> Result<(), FsError> {
        self.copy_file_calls.fetch_add(1, Ordering::SeqCst);
        self.inner.copy_file(from, to)
    }

    fn set_mtime(&self, path: &Path, mtime: OffsetDateTime) -> Result<(), FsError> {
        self.inner.set_mtime(path, mtime)
    }

    fn writable(&self, path: &Path) -> bool {
        self.inner.writable(path)
    }
}
