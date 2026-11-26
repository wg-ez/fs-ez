//! Cas d'usage d'authentification : utilisateur, sessions et jetons d'API.

use fs_ez_core::{ApiTokenId, UserId};
use fs_ez_store::{ApiTokenRecord, StoreError, UserRecord, normalize_api_token_name};
use time::{Duration, OffsetDateTime};

use crate::app::App;
use crate::error::AppError;
use crate::password::{hash_password, verify_or_dummy};
use crate::token::{hash_token, is_valid_token_format, random_token};

/// Durée de vie d'une session web.
const SESSION_TTL: Duration = Duration::days(30);
/// Fréquence maximale d'écriture de `last_seen_at`.
const LAST_SEEN_REFRESH: Duration = Duration::minutes(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal {
    pub username: String,
    pub user_id: UserId,
    /// Generation du credential qui a autorise la session.
    pub credential_generation: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginSession {
    pub token: String,
    pub expires_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreatedApiToken {
    pub record: ApiTokenRecord,
    pub token: String,
}

/// Hash du jeton uniquement si son format est valide, `None` sinon.
fn hashed_token(token: &str) -> Option<String> {
    is_valid_token_format(token).then(|| hash_token(token))
}

impl App {
    pub fn has_user(&self) -> Result<bool, AppError> {
        Ok(self.store.get_user()?.is_some())
    }

    /// Generation courante du credential, 0 tant qu'aucun utilisateur n'existe.
    pub fn credential_generation(&self) -> Result<i64, AppError> {
        Ok(self
            .store
            .get_user()?
            .map_or(0, |user| user.credential_generation))
    }

    /// Premier boot uniquement : le store tranche, un utilisateur déjà présent est un conflit.
    pub fn init_user(&self, username: &str, password: &str) -> Result<UserRecord, AppError> {
        let password_hash = hash_password(password)?;
        self.store
            .upsert_user(username, &password_hash, self.now())
            .map_err(|error| match error {
                StoreError::Conflict(message) => AppError::Conflict(message),
                other => AppError::Store(other),
            })
    }

    /// Remplace le mot de passe et révoque toutes les sessions et tous les jetons d'API.
    pub fn reset_user(&self, username: &str, password: &str) -> Result<UserRecord, AppError> {
        let password_hash = hash_password(password)?;
        Ok(self
            .store
            .reset_user_credentials(username, &password_hash, self.now())?)
    }

    /// Nom d'utilisateur inconnu : même coût Argon2, jamais d'indice par le temps.
    pub fn verify_user(&self, username: &str, password: &str) -> Result<bool, AppError> {
        Ok(self.authenticated_user(username, password)?.is_some())
    }

    // Renvoie l'utilisateur lu avant Argon2, sa generation de credential doit etre capturee avant le reset.
    fn authenticated_user(
        &self,
        username: &str,
        password: &str,
    ) -> Result<Option<UserRecord>, AppError> {
        let user = self.store.get_user()?;
        let wanted = username.trim();
        let hash = user
            .as_ref()
            .filter(|user| user.username == wanted)
            .map(|user| user.password_hash.as_str());
        if verify_or_dummy(hash, password)? {
            Ok(user)
        } else {
            Ok(None)
        }
    }

    pub fn login(&self, username: &str, password: &str) -> Result<LoginSession, AppError> {
        let Some(user) = self.authenticated_user(username, password)? else {
            return Err(AppError::Invalid("invalid credentials".to_string()));
        };
        let token = random_token()?;
        let now = self.now();
        let expires_at = now + SESSION_TTL;
        self.store.create_session(
            &hash_token(&token),
            user.credential_generation,
            now,
            expires_at,
        )?;
        Ok(LoginSession { token, expires_at })
    }

    pub fn validate_session(&self, token: &str) -> Result<Option<Principal>, AppError> {
        let Some(hash) = hashed_token(token) else {
            return Ok(None);
        };
        let Some(session) = self.store.find_session(&hash)? else {
            return Ok(None);
        };
        let now = self.now();
        if session.expires_at <= now {
            return Ok(None);
        }
        let Some(user) = self.store.get_user()? else {
            return Ok(None);
        };
        // Un login parti avant le reset garde l'ancienne generation, sa session ne doit jamais valider.
        if session.credential_generation != user.credential_generation {
            return Ok(None);
        }
        if now - session.last_seen_at >= LAST_SEEN_REFRESH {
            self.store.touch_session(session.id, now)?;
        }
        Ok(Some(Principal {
            username: user.username,
            user_id: user.id,
            credential_generation: user.credential_generation,
        }))
    }

    pub fn logout(&self, token: &str) -> Result<(), AppError> {
        self.store.delete_session_by_hash(&hash_token(token))?;
        Ok(())
    }

    pub fn purge_expired_sessions(&self) -> Result<u64, AppError> {
        Ok(self.store.purge_expired_sessions(self.now())?)
    }

    /// Usage CLI/local : enregistre la generation courante de l'utilisateur.
    pub fn create_api_token(
        &self,
        name: &str,
        expires_at: Option<OffsetDateTime>,
    ) -> Result<CreatedApiToken, AppError> {
        let generation = self.credential_generation()?;
        self.mint_api_token(name, expires_at, generation)
    }

    /// Web authentifie : refuse si la generation capturee a l'authentification n'est plus la courante.
    pub fn create_api_token_for_generation(
        &self,
        name: &str,
        expires_at: Option<OffsetDateTime>,
        expected_generation: i64,
    ) -> Result<CreatedApiToken, AppError> {
        self.mint_api_token(name, expires_at, expected_generation)
    }

    fn mint_api_token(
        &self,
        name: &str,
        expires_at: Option<OffsetDateTime>,
        credential_generation: i64,
    ) -> Result<CreatedApiToken, AppError> {
        let Some(name) = normalize_api_token_name(name) else {
            return Err(AppError::Invalid(
                "api token name must be 1..=100 characters".to_string(),
            ));
        };
        let now = self.now();
        if let Some(expires_at) = expires_at
            && expires_at <= now
        {
            return Err(AppError::Invalid(
                "api token expiration must be in the future".to_string(),
            ));
        }
        let token = random_token()?;
        let record = self.store.create_api_token(
            &name,
            &hash_token(&token),
            credential_generation,
            now,
            expires_at,
        )?;
        Ok(CreatedApiToken { record, token })
    }

    pub fn list_api_tokens(&self) -> Result<Vec<ApiTokenRecord>, AppError> {
        Ok(self.store.list_api_tokens()?)
    }

    pub fn revoke_api_token(&self, id: ApiTokenId) -> Result<bool, AppError> {
        Ok(self.store.revoke_api_token(id)?)
    }

    pub fn validate_api_token(&self, token: &str) -> Result<Option<ApiTokenRecord>, AppError> {
        let Some(hash) = hashed_token(token) else {
            return Ok(None);
        };
        let Some(mut record) = self.store.find_api_token_by_hash(&hash)? else {
            return Ok(None);
        };
        let current = self.credential_generation()?;
        if record.credential_generation != current {
            return Ok(None);
        }
        let now = self.now();
        if record
            .expires_at
            .is_some_and(|expires_at| expires_at <= now)
        {
            return Ok(None);
        }
        self.store.touch_api_token(record.id, now)?;
        record.last_used_at = Some(now);
        Ok(Some(record))
    }
}
