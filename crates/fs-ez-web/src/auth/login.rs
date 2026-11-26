//! Limitation de débit de `POST /login`, en mémoire et par adresse IP.

use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::time::{Duration, Instant};

/// Fenêtre glissante de comptage, exposée pour l'en-tête `Retry-After`.
pub(crate) const WINDOW_SECONDS: u64 = 15 * 60;
const WINDOW: Duration = Duration::from_secs(WINDOW_SECONDS);
/// Échecs autorisés par fenêtre et par IP.
const MAX_FAILURES: u32 = 5;

#[derive(Debug)]
pub struct LoginLimiter {
    window: Duration,
    max_failures: u32,
    failures: HashMap<IpAddr, VecDeque<Instant>>,
}

impl Default for LoginLimiter {
    fn default() -> Self {
        Self::new(WINDOW, MAX_FAILURES)
    }
}

impl LoginLimiter {
    pub fn new(window: Duration, max_failures: u32) -> Self {
        Self {
            window,
            max_failures,
            failures: HashMap::new(),
        }
    }

    /// Réserve atomiquement une tentative : `false` si la fenêtre est déjà pleine.
    /// Le créneau reste compté jusqu'à `clear` (succès) ou expiration de la fenêtre.
    pub fn check_and_record(&mut self, ip: IpAddr, now: Instant) -> bool {
        self.purge(now);
        let stamps = self.failures.entry(ip).or_default();
        if (stamps.len() as u32) >= self.max_failures {
            return false;
        }
        stamps.push_back(now);
        true
    }

    pub fn clear(&mut self, ip: IpAddr) {
        self.failures.remove(&ip);
    }

    fn purge(&mut self, now: Instant) {
        let window = self.window;
        self.failures.retain(|_, stamps| {
            while stamps.front().is_some_and(|stamp| {
                now.checked_duration_since(*stamp)
                    .is_some_and(|elapsed| elapsed >= window)
            }) {
                stamps.pop_front();
            }
            !stamps.is_empty()
        });
    }
}

#[cfg(test)]
mod tests;
