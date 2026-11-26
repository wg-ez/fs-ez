//! État partagé du routeur : backend, configuration, locales, événements et limiteur.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::auth::LoginLimiter;
use crate::backend::Backend;
use crate::config::WebConfig;
use crate::dav::DavService;
use crate::locales::Locales;

/// Capacité du canal : les abonnés lents perdent des événements, jamais le serveur.
const EVENT_CAPACITY: usize = 256;

pub struct AppState<B: Backend> {
    pub backend: Arc<B>,
    pub config: Arc<WebConfig>,
    pub locales: Arc<Locales>,
    pub events: tokio::sync::broadcast::Sender<Event>,
    pub login_limiter: Arc<tokio::sync::Mutex<LoginLimiter>>,
    /// Créneau unique de setup : permis owned porté par la tâche bloquante.
    pub setup_gate: Arc<tokio::sync::Semaphore>,
    /// Un seul scan inbox en arrière-plan à la fois.
    pub rescan_running: Arc<AtomicBool>,
    pub dav: DavService,
}

#[derive(Debug, Clone)]
pub enum Event {
    DataChanged,
    InboxChanged,
    ImportFinished { imported: usize, duplicates: usize },
    Resync,
}

impl<B: Backend> AppState<B> {
    pub fn new(backend: Arc<B>, config: Arc<WebConfig>, locales: Arc<Locales>) -> Self {
        let (events, _receiver) = tokio::sync::broadcast::channel(EVENT_CAPACITY);
        let dav = crate::dav::service(backend.app(), config.max_upload, events.clone());
        Self {
            backend,
            config,
            locales,
            events,
            login_limiter: Arc::new(tokio::sync::Mutex::new(LoginLimiter::default())),
            setup_gate: Arc::new(tokio::sync::Semaphore::new(1)),
            rescan_running: Arc::new(AtomicBool::new(false)),
            dav,
        }
    }
}

// Dérivation manuelle : `Backend` n'exige pas `Clone`, `Arc<B>` suffit.
impl<B: Backend> Clone for AppState<B> {
    fn clone(&self) -> Self {
        Self {
            backend: Arc::clone(&self.backend),
            config: Arc::clone(&self.config),
            locales: Arc::clone(&self.locales),
            events: self.events.clone(),
            login_limiter: Arc::clone(&self.login_limiter),
            setup_gate: Arc::clone(&self.setup_gate),
            rescan_running: Arc::clone(&self.rescan_running),
            dav: self.dav.clone(),
        }
    }
}
