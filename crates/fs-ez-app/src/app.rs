//! Contexte applicatif partagé par la CLI et le web.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use fs_ez_core::{FileId, Settings};
use fs_ez_fingerprint::Fingerprinter;
use fs_ez_fs::{Fs, Root};
use fs_ez_store::Store;
use time::OffsetDateTime;

use crate::error::AppError;

/// Bandes de verrous : borne la mémoire quel que soit le nombre d'ids.
const LOCK_STRIPES: usize = 64;

#[derive(Clone)]
pub struct App {
    pub store: Arc<dyn Store>,
    pub fs: Arc<dyn Fs>,
    pub fingerprints: Arc<dyn Fingerprinter>,
    pub root: Root,
    /// Chemin effectif choisi par l'adaptateur, `None` = résolution par variables d'environnement.
    pub db_path: Option<PathBuf>,
    /// Sérialise rename et remplacement de contenu d'un même id dans ce processus.
    mutation_locks: Arc<Vec<Mutex<()>>>,
}

impl App {
    pub fn new(
        store: Arc<dyn Store>,
        fs: Arc<dyn Fs>,
        fingerprints: Arc<dyn Fingerprinter>,
        root: Root,
    ) -> Self {
        Self {
            store,
            fs,
            fingerprints,
            root,
            db_path: None,
            mutation_locks: Arc::new((0..LOCK_STRIPES).map(|_| Mutex::new(())).collect()),
        }
    }

    /// `--db` fige le chemin de la base pour `status`.
    pub fn with_db_path(mut self, db_path: PathBuf) -> Self {
        self.db_path = Some(db_path);
        self
    }

    /// Non mockable en v1 : les tests comparent des bornes temporelles.
    pub fn now(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }

    /// Verrou de mutation d'un fichier, poison ignoré : disque et base restent la vérité.
    pub fn lock_file(&self, id: FileId) -> MutexGuard<'_, ()> {
        let stripe = id.0.unsigned_abs() as usize % self.mutation_locks.len();
        self.mutation_locks[stripe]
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    pub fn settings(&self) -> Result<Settings, AppError> {
        let pairs = self.store.all_settings()?;
        Ok(Settings::from_pairs(
            pairs
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str())),
        ))
    }
}
