//! Cache TTL borné partagé par les jetons d'API et les dossiers virtuels DAV.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// TTL et capacité fixes, purge des expirés puis éviction du plus ancien avant insertion.
#[derive(Debug)]
pub(crate) struct TtlMap<V> {
    entries: Arc<Mutex<HashMap<String, (V, Instant)>>>,
    ttl: Duration,
    capacity: usize,
}

impl<V> Clone for TtlMap<V> {
    fn clone(&self) -> Self {
        Self {
            entries: Arc::clone(&self.entries),
            ttl: self.ttl,
            capacity: self.capacity,
        }
    }
}

impl<V> TtlMap<V> {
    pub(crate) fn new(ttl: Duration, capacity: usize) -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
            ttl,
            capacity,
        }
    }

    pub(crate) fn insert(&self, key: String, value: V) {
        let Ok(mut entries) = self.entries.lock() else {
            return;
        };
        let now = Instant::now();
        entries.retain(|_, (_, seen)| now.duration_since(*seen) < self.ttl);
        if entries.len() >= self.capacity
            && let Some(oldest) = entries
                .iter()
                .min_by_key(|(_, (_, seen))| *seen)
                .map(|(key, _)| key.clone())
        {
            entries.remove(&oldest);
        }
        entries.insert(key, (value, now));
    }

    /// Lecture sans purge ni rafraîchissement, l'horodatage reste celui de l'insertion.
    pub(crate) fn get(&self, key: &str) -> Option<V>
    where
        V: Clone,
    {
        let entries = self.entries.lock().ok()?;
        entries
            .get(key)
            .filter(|(_, seen)| seen.elapsed() < self.ttl)
            .map(|(value, _)| value.clone())
    }

    pub(crate) fn remove(&self, key: &str) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.remove(key);
        }
    }

    pub(crate) fn clear(&self) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.clear();
        }
    }
}
