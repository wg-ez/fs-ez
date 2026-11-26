//! Port de persistance de l'authentification : utilisateur, sessions et jetons.

use fs_ez_core::{ApiTokenId, SessionId};
use time::OffsetDateTime;

use crate::error::StoreResult;
use crate::types::{ApiTokenRecord, SessionRecord, UserRecord};

pub trait AuthStore {
    fn get_user(&self) -> StoreResult<Option<UserRecord>>;
    /// Création unique de l'id `1`, un utilisateur déjà présent donne `Conflict` sans mise à jour.
    fn upsert_user(
        &self,
        username: &str,
        password_hash: &str,
        at: OffsetDateTime,
    ) -> StoreResult<UserRecord>;
    /// Remplace le mot de passe et revoque sessions + jetons dans une meme transaction immediate.
    fn reset_user_credentials(
        &self,
        username: &str,
        password_hash: &str,
        at: OffsetDateTime,
    ) -> StoreResult<UserRecord>;

    /// `credential_generation` est celle lue avant la verification du mot de passe.
    fn create_session(
        &self,
        token_hash: &str,
        credential_generation: i64,
        created_at: OffsetDateTime,
        expires_at: OffsetDateTime,
    ) -> StoreResult<SessionId>;
    fn find_session(&self, token_hash: &str) -> StoreResult<Option<SessionRecord>>;
    fn touch_session(&self, id: SessionId, at: OffsetDateTime) -> StoreResult<()>;
    fn delete_session(&self, id: SessionId) -> StoreResult<bool>;
    fn delete_session_by_hash(&self, token_hash: &str) -> StoreResult<bool>;
    fn delete_all_sessions(&self) -> StoreResult<u64>;
    fn purge_expired_sessions(&self, now: OffsetDateTime) -> StoreResult<u64>;

    /// Insertion atomique sous verrou immediat, refuse `Conflict` si `credential_generation` n'est plus celle de l'utilisateur.
    fn create_api_token(
        &self,
        name: &str,
        token_hash: &str,
        credential_generation: i64,
        created_at: OffsetDateTime,
        expires_at: Option<OffsetDateTime>,
    ) -> StoreResult<ApiTokenRecord>;
    fn list_api_tokens(&self) -> StoreResult<Vec<ApiTokenRecord>>;
    fn find_api_token_by_hash(&self, token_hash: &str) -> StoreResult<Option<ApiTokenRecord>>;
    fn touch_api_token(&self, id: ApiTokenId, at: OffsetDateTime) -> StoreResult<()>;
    fn revoke_api_token(&self, id: ApiTokenId) -> StoreResult<bool>;
    fn delete_all_api_tokens(&self) -> StoreResult<u64>;
}
