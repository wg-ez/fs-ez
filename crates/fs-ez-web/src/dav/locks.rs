//! Expiration des verrous WebDAV au-dessus de `MemLs`, qui ne consulte jamais `timeout_at`.
//!
//! Chaque verrou créé est suivi (chemin, jeton, échéance) puis retiré du `MemLs` sous-jacent dès qu'il expire.
//! `lock`, `unlock`, `refresh`, `check`, `discover` et `delete` balaient les expirés avant d'agir.
//! Un verrou sans `Timeout` reçoit une échéance finie pour qu'un client abandonné ne bloque pas à vie.
//! Le nombre de verrous vivants est plafonné.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime};

use dav_server::davpath::DavPath;
use dav_server::ls::{DavLock, DavLockSystem, LsFuture};
use dav_server::memls::MemLs;
use futures_util::FutureExt;
use xmltree::Element;

/// Échéance imposée quand le client ne demande pas de timeout, valeur affichée dans la réponse LOCK.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(600);
/// Plafond de verrous vivants, au-delà toute création échoue en 423.
const MAX_LOCKS: usize = 1024;

#[derive(Debug, Clone)]
pub(crate) struct ExpiringLs {
    inner: MemLs,
    state: Arc<Mutex<State>>,
}

#[derive(Debug, Default)]
struct State {
    locks: HashMap<String, DavLock>,
}

impl ExpiringLs {
    pub(crate) fn new() -> Box<Self> {
        Box::new(Self {
            inner: *MemLs::new(),
            state: Arc::new(Mutex::new(State::default())),
        })
    }

    /// Retire les verrous échus de la table puis du `MemLs` sous-jacent.
    async fn reap(&self) {
        let expired: Vec<DavLock> = {
            let mut state = self.state();
            let now = SystemTime::now();
            let expired: Vec<DavLock> = state
                .locks
                .values()
                .filter(|lock| expired(lock, now))
                .cloned()
                .collect();
            for lock in &expired {
                state.locks.remove(&lock.token);
            }
            expired
        };
        for lock in expired {
            self.inner.unlock(&lock.path, &lock.token).await.ok();
        }
    }

    /// Suit le verrou rendu par `MemLs`, l'échéance et le jeton restent ceux d'origine.
    fn remember(&self, lock: &DavLock) {
        self.state().locks.insert(lock.token.clone(), lock.clone());
    }

    /// Premier conflit possible quand le plafond est atteint, `None` seulement si la table est vide.
    fn capped(&self) -> Option<DavLock> {
        let state = self.state();
        if state.locks.len() < MAX_LOCKS {
            return None;
        }
        state.locks.values().next().cloned()
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl DavLockSystem for ExpiringLs {
    fn lock(
        &'_ self,
        path: &DavPath,
        principal: Option<&str>,
        owner: Option<&Element>,
        timeout: Option<Duration>,
        shared: bool,
        deep: bool,
    ) -> LsFuture<'_, Result<DavLock, DavLock>> {
        let path = path.clone();
        let principal = principal.map(str::to_string);
        let owner = owner.cloned();
        let timeout = timeout.or(Some(DEFAULT_TIMEOUT));
        async move {
            self.reap().await;
            if let Some(conflict) = self.capped() {
                return Err(conflict);
            }
            let result = self
                .inner
                .lock(
                    &path,
                    principal.as_deref(),
                    owner.as_ref(),
                    timeout,
                    shared,
                    deep,
                )
                .await;
            if let Ok(lock) = &result {
                self.remember(lock);
            }
            result
        }
        .boxed()
    }

    fn unlock(&'_ self, path: &DavPath, token: &str) -> LsFuture<'_, Result<(), ()>> {
        let path = path.clone();
        let token = token.to_string();
        async move {
            self.reap().await;
            let result = self.inner.unlock(&path, &token).await;
            if result.is_ok() {
                self.state().locks.remove(&token);
            }
            result
        }
        .boxed()
    }

    fn refresh(
        &'_ self,
        path: &DavPath,
        token: &str,
        timeout: Option<Duration>,
    ) -> LsFuture<'_, Result<DavLock, ()>> {
        let path = path.clone();
        let token = token.to_string();
        let timeout = timeout.or(Some(DEFAULT_TIMEOUT));
        async move {
            self.reap().await;
            let result = self.inner.refresh(&path, &token, timeout).await;
            if let Ok(lock) = &result {
                self.remember(lock);
            }
            result
        }
        .boxed()
    }

    fn check(
        &'_ self,
        path: &DavPath,
        principal: Option<&str>,
        ignore_principal: bool,
        deep: bool,
        submitted_tokens: &[String],
    ) -> LsFuture<'_, Result<(), DavLock>> {
        let path = path.clone();
        let principal = principal.map(str::to_string);
        let submitted_tokens = submitted_tokens.to_vec();
        async move {
            self.reap().await;
            self.inner
                .check(
                    &path,
                    principal.as_deref(),
                    ignore_principal,
                    deep,
                    &submitted_tokens,
                )
                .await
        }
        .boxed()
    }

    fn discover(&'_ self, path: &DavPath) -> LsFuture<'_, Vec<DavLock>> {
        let path = path.clone();
        async move {
            self.reap().await;
            let mut locks = self.inner.discover(&path).await;
            let now = SystemTime::now();
            locks.retain(|lock| !expired(lock, now));
            locks
        }
        .boxed()
    }

    fn delete(&'_ self, path: &DavPath) -> LsFuture<'_, Result<(), ()>> {
        let path = path.clone();
        async move {
            self.reap().await;
            let result = self.inner.delete(&path).await;
            self.state()
                .locks
                .retain(|_, lock| !covers(&path, &lock.path));
            result
        }
        .boxed()
    }
}

/// Verrou dont l'échéance est atteinte ou dépassée, sans échéance jamais échu.
fn expired(lock: &DavLock, now: SystemTime) -> bool {
    lock.timeout_at.is_some_and(|at| at <= now)
}

/// `parent` couvre `child` si les segments de `child` commencent par ceux de `parent`.
fn covers(parent: &DavPath, child: &DavPath) -> bool {
    let parent = parent.as_bytes();
    let parent = parent.strip_suffix(b"/").unwrap_or(parent);
    let child = child.as_bytes();
    if parent.is_empty() {
        return true;
    }
    child == parent
        || child
            .strip_prefix(parent)
            .is_some_and(|rest| rest.starts_with(b"/"))
}

#[cfg(test)]
mod tests;
