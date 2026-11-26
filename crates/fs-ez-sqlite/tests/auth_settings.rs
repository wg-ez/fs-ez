//! Tests d'integration de `AuthStore` et `SettingsStore` sur base memoire.

use fs_ez_core::{ApiTokenId, SessionId, UserId};
use fs_ez_store::{AuthStore, SettingsStore, StoreError};
use time::macros::datetime;

mod common;

use common::store;

/// Hash factice mais valide : 64 caracteres hexadecimaux.
fn hash(seed: u8) -> String {
    format!("{seed:02x}").repeat(32)
}

#[test]
fn user_creation_is_one_shot() {
    let store = store();
    assert!(store.get_user().expect("lecture").is_none());

    let created = datetime!(2026-01-01 00:00 UTC);
    let updated = datetime!(2026-02-01 00:00 UTC);
    let first = store
        .upsert_user("alice", "argon2-hash-1", created)
        .expect("insertion user");
    assert_eq!(first.id, UserId(1));
    assert_eq!(first.username, "alice");
    assert_eq!(first.password_hash, "argon2-hash-1");
    assert_eq!(first.created_at, created);
    assert_eq!(first.updated_at, created);

    assert!(matches!(
        store.upsert_user("  alice2  ", "argon2-hash-2", updated),
        Err(StoreError::Conflict(_))
    ));
    let unchanged = store.get_user().expect("lecture").expect("ligne");
    assert_eq!(unchanged.id, UserId(1));
    assert_eq!(unchanged.username, "alice");
    assert_eq!(unchanged.password_hash, "argon2-hash-1");
    assert_eq!(unchanged.created_at, created);
    assert_eq!(unchanged.updated_at, created);
}

#[test]
fn user_username_is_validated() {
    let store = store();
    let at = datetime!(2026-01-01 00:00 UTC);
    for name in ["", "   ", &"x".repeat(65)] {
        assert!(matches!(
            store.upsert_user(name, "hash", at),
            Err(StoreError::Invalid(_))
        ));
    }
    assert!(store.get_user().expect("lecture").is_none());
    let record = store
        .upsert_user(&"x".repeat(64), "hash", at)
        .expect("64 caracteres acceptes");
    assert_eq!(record.username.chars().count(), 64);
}

#[test]
fn session_round_trip_and_touch() {
    let store = store();
    let created = datetime!(2026-01-01 00:00 UTC);
    let expires = datetime!(2026-02-01 00:00 UTC);
    let seen = datetime!(2026-01-01 12:00 UTC);

    let id = store
        .create_session(&hash(1), 0, created, expires)
        .expect("creation session");
    let found = store
        .find_session(&hash(1))
        .expect("recherche session")
        .expect("session presente");
    assert_eq!(found.id, id);
    assert_eq!(found.token_hash, hash(1));
    assert_eq!(found.created_at, created);
    assert_eq!(found.expires_at, expires);
    assert_eq!(found.last_seen_at, created);

    store.touch_session(id, seen).expect("touch session");
    let touched = store
        .find_session(&hash(1))
        .expect("recherche session")
        .expect("session presente");
    assert_eq!(touched.last_seen_at, seen);
    assert_eq!(touched.created_at, created);

    store
        .touch_session(SessionId(999), seen)
        .expect("touch silencieux sur session absente");
}

#[test]
fn session_delete_by_id_and_hash() {
    let store = store();
    let created = datetime!(2026-01-01 00:00 UTC);
    let expires = datetime!(2026-02-01 00:00 UTC);

    let id = store
        .create_session(&hash(2), 0, created, expires)
        .expect("creation session");
    assert!(store.delete_session(id).expect("suppression par id"));
    assert!(!store.delete_session(id).expect("suppression deja faite"));
    assert!(store.find_session(&hash(2)).expect("recherche").is_none());

    store
        .create_session(&hash(3), 0, created, expires)
        .expect("creation session");
    assert!(
        store
            .delete_session_by_hash(&hash(3))
            .expect("suppression par hash")
    );
    assert!(
        !store
            .delete_session_by_hash(&hash(3))
            .expect("suppression deja faite")
    );
}

#[test]
fn session_purge_removes_only_past_expirations() {
    let store = store();
    let created = datetime!(2026-01-01 00:00 UTC);
    let now = datetime!(2026-01-15 00:00 UTC);

    store
        .create_session(&hash(4), 0, created, datetime!(2026-01-01 00:00 UTC))
        .expect("session expiree");
    store
        .create_session(&hash(5), 0, created, now)
        .expect("session limite");
    store
        .create_session(&hash(6), 0, created, datetime!(2026-02-01 00:00 UTC))
        .expect("session valide");

    assert_eq!(store.purge_expired_sessions(now).expect("purge"), 1);
    assert!(store.find_session(&hash(4)).expect("recherche").is_none());
    assert!(store.find_session(&hash(5)).expect("recherche").is_some());
    assert!(store.find_session(&hash(6)).expect("recherche").is_some());
    assert_eq!(store.delete_all_sessions().expect("purge totale"), 2);
}

#[test]
fn session_hashes_are_validated() {
    let store = store();
    let at = datetime!(2026-01-01 00:00 UTC);
    let expires = datetime!(2026-02-01 00:00 UTC);

    for bad in [
        "".to_string(),
        "abc".to_string(),
        "z".repeat(64),
        "é".repeat(64),
    ] {
        assert!(matches!(
            store.create_session(&bad, 0, at, expires),
            Err(StoreError::Invalid(_))
        ));
    }
    assert!(
        store
            .find_session("not-a-hash")
            .expect("recherche")
            .is_none()
    );
    assert!(
        !store
            .delete_session_by_hash("not-a-hash")
            .expect("suppression")
    );
    assert_eq!(store.delete_all_sessions().expect("purge totale"), 0);

    let mixed_case = "A1".repeat(32);
    store
        .create_session(&mixed_case, 0, at, expires)
        .expect("hexadecimaux majuscules acceptes");
}

#[test]
fn reset_user_credentials_increments_generation_and_revokes() {
    let store = store();
    let created = datetime!(2026-01-01 00:00 UTC);
    let updated = datetime!(2026-02-01 00:00 UTC);
    store
        .upsert_user("user", "old-hash", created)
        .expect("creation user");
    store
        .create_session(&hash(20), 0, created, datetime!(2026-02-01 00:00 UTC))
        .expect("creation session");
    store
        .create_api_token("ci", &hash(21), 0, created, None)
        .expect("creation jeton");

    let record = store
        .reset_user_credentials("user", "new-hash", updated)
        .expect("reset");
    assert_eq!(record.password_hash, "new-hash");
    assert_eq!(record.credential_generation, 1);
    assert_eq!(record.created_at, created);
    assert_eq!(record.updated_at, updated);
    assert!(store.find_session(&hash(20)).expect("session").is_none());
    assert!(store.list_api_tokens().expect("jetons").is_empty());

    let second = store
        .reset_user_credentials("user", "newer-hash", updated)
        .expect("second reset");
    assert_eq!(second.credential_generation, 2);
}

#[test]
fn api_token_insert_refuses_a_stale_generation() {
    let store = store();
    let at = datetime!(2026-01-01 00:00 UTC);
    store
        .upsert_user("user", "old-hash", at)
        .expect("creation user");

    assert!(matches!(
        store.create_api_token("stale", &hash(30), 1, at, None),
        Err(StoreError::Conflict(_))
    ));
    assert!(store.list_api_tokens().expect("jetons").is_empty());

    let created = store
        .create_api_token("ok", &hash(31), 0, at, None)
        .expect("generation courante acceptee");
    assert_eq!(created.credential_generation, 0);
}

#[test]
fn api_token_round_trip() {
    let store = store();
    let created = datetime!(2026-01-01 00:00 UTC);
    let used = datetime!(2026-01-02 00:00 UTC);

    let token = store
        .create_api_token("  CI  ", &hash(7), 0, created, None)
        .expect("creation jeton");
    assert_eq!(token.id, ApiTokenId(1));
    assert_eq!(token.name, "CI");
    assert_eq!(token.token_hash, hash(7));
    assert_eq!(token.created_at, created);
    assert_eq!(token.expires_at, None);
    assert_eq!(token.last_used_at, None);
    assert_eq!(token.credential_generation, 0);

    assert_eq!(
        store
            .find_api_token_by_hash(&hash(7))
            .expect("recherche jeton"),
        Some(token.clone())
    );

    store.touch_api_token(token.id, used).expect("touch jeton");
    let touched = store
        .find_api_token_by_hash(&hash(7))
        .expect("recherche jeton")
        .expect("jeton present");
    assert_eq!(touched.last_used_at, Some(used));

    assert!(store.revoke_api_token(token.id).expect("revocation"));
    assert!(
        !store
            .revoke_api_token(token.id)
            .expect("revocation deja faite")
    );
    assert!(
        store
            .find_api_token_by_hash(&hash(7))
            .expect("recherche jeton")
            .is_none()
    );
}

#[test]
fn api_tokens_are_listed_newest_first() {
    let store = store();
    let early = datetime!(2026-01-01 00:00 UTC);
    let late = datetime!(2026-01-02 00:00 UTC);

    store
        .create_api_token("early", &hash(8), 0, early, None)
        .expect("creation jeton");
    store
        .create_api_token(
            "late",
            &hash(9),
            0,
            late,
            Some(datetime!(2026-03-01 00:00 UTC)),
        )
        .expect("creation jeton");

    let names: Vec<String> = store
        .list_api_tokens()
        .expect("liste jetons")
        .into_iter()
        .map(|token| token.name)
        .collect();
    assert_eq!(names, vec!["late".to_string(), "early".to_string()]);
    assert_eq!(store.delete_all_api_tokens().expect("purge jetons"), 2);
    assert!(store.list_api_tokens().expect("liste jetons").is_empty());
}

#[test]
fn api_token_name_and_hash_conflicts() {
    let store = store();
    let at = datetime!(2026-01-01 00:00 UTC);
    store
        .create_api_token("cli", &hash(10), 0, at, None)
        .expect("creation jeton");

    assert!(matches!(
        store.create_api_token("cli", &hash(11), 0, at, None),
        Err(StoreError::Conflict(_))
    ));
    assert!(matches!(
        store.create_api_token("other", &hash(10), 0, at, None),
        Err(StoreError::Conflict(_))
    ));
}

#[test]
fn api_token_inputs_are_validated() {
    let store = store();
    let at = datetime!(2026-01-01 00:00 UTC);

    for name in ["", "   ", &"x".repeat(101)] {
        assert!(matches!(
            store.create_api_token(name, &hash(12), 0, at, None),
            Err(StoreError::Invalid(_))
        ));
    }
    assert!(matches!(
        store.create_api_token("bad", "nope", 0, at, None),
        Err(StoreError::Invalid(_))
    ));
    assert!(
        store
            .find_api_token_by_hash("nope")
            .expect("recherche jeton")
            .is_none()
    );
}

#[test]
fn settings_round_trip() {
    let store = store();
    assert!(
        store
            .get_setting("theme")
            .expect("lecture reglage")
            .is_none()
    );

    store.set_setting("theme", "dark").expect("ecriture");
    assert_eq!(
        store.get_setting("theme").expect("lecture reglage"),
        Some("dark".to_string())
    );

    store.set_setting("theme", "light").expect("ecrasement");
    store.set_setting("locale", "fr").expect("ecriture");
    store.set_setting("a_first", "1").expect("ecriture");
    assert_eq!(
        store.all_settings().expect("liste reglages"),
        vec![
            ("a_first".to_string(), "1".to_string()),
            ("locale".to_string(), "fr".to_string()),
            ("theme".to_string(), "light".to_string()),
        ]
    );

    assert!(store.delete_setting("theme").expect("suppression"));
    assert!(
        !store
            .delete_setting("theme")
            .expect("suppression deja faite")
    );
    assert!(
        store
            .get_setting("theme")
            .expect("lecture reglage")
            .is_none()
    );
}

#[test]
fn settings_inputs_are_bounded() {
    let store = store();
    for key in ["", "   ", &"k".repeat(101)] {
        assert!(matches!(
            store.set_setting(key, "value"),
            Err(StoreError::Invalid(_))
        ));
    }

    let max_value = "v".repeat(4096);
    store
        .set_setting("max", &max_value)
        .expect("valeur maximale");
    assert_eq!(
        store.get_setting("max").expect("lecture reglage"),
        Some(max_value)
    );
    assert!(matches!(
        store.set_setting("too_long", &"v".repeat(4097)),
        Err(StoreError::Invalid(_))
    ));
}
