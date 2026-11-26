use std::sync::Arc;

use fs_ez_app::{AppError, MIN_PASSWORD_LEN};
use fs_ez_sqlite::{SqliteStore, db};
use tempfile::TempDir;
use time::Duration;

mod common;

use common::{app_with_store, test_app};

const PASSWORD: &str = "correct-horse-battery";

#[test]
fn init_user_then_login_logout() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    assert!(!app.has_user().expect("user"));
    assert!(!app.verify_user("user", PASSWORD).expect("absent"));

    let user = app.init_user("  user  ", PASSWORD).expect("init");
    assert_eq!(user.username, "user");
    assert!(app.has_user().expect("user"));

    let session = app.login("user", PASSWORD).expect("login");
    assert!(!session.token.is_empty());
    assert_eq!(session.token.len(), 43);
    assert!(session.expires_at > app.now());

    let principal = app
        .validate_session(&session.token)
        .expect("validation")
        .expect("session connue");
    assert_eq!(principal.username, "user");
    assert_eq!(principal.user_id, user.id);
    assert!(
        app.validate_session("not-a-token")
            .expect("format")
            .is_none()
    );

    app.logout(&session.token).expect("logout");
    assert!(
        app.validate_session(&session.token)
            .expect("validation")
            .is_none()
    );
    assert!(app.validate_session("nope").is_ok());
}

#[test]
fn init_user_twice_is_a_conflict() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    app.init_user("user", PASSWORD).expect("init");
    assert!(matches!(
        app.init_user("other", PASSWORD),
        Err(AppError::Conflict(_))
    ));
    // Hachage avant l'insertion : un mot de passe trop court est refuse avant tout conflit.
    assert!(matches!(
        app.init_user("user", "short"),
        Err(AppError::Invalid(_))
    ));
    assert_eq!(MIN_PASSWORD_LEN, 12);
}

// Deuxieme setup refuse : aucun ecrasement, la session du premier compte continue d'authentifier l'identite d'origine.
#[test]
fn second_init_is_refused_and_first_session_stays_valid() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    app.init_user("alice", "alice-password-123")
        .expect("premier setup");
    let session = app.login("alice", "alice-password-123").expect("login");

    assert!(matches!(
        app.init_user("bravo", "bravo-password-123"),
        Err(AppError::Conflict(_))
    ));

    let principal = app
        .validate_session(&session.token)
        .expect("validation")
        .expect("session d'origine toujours valide");
    assert_eq!(principal.username, "alice");
    assert!(
        app.verify_user("alice", "alice-password-123")
            .expect("verification")
    );
    assert!(
        !app.verify_user("bravo", "bravo-password-123")
            .expect("verification")
    );
}

#[test]
fn wrong_password_and_unknown_username_are_refused() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    app.init_user("user", PASSWORD).expect("init");

    assert!(
        !app.verify_user("user", "wrong-password12")
            .expect("vérification")
    );
    assert!(!app.verify_user("root", PASSWORD).expect("vérification"));
    assert!(matches!(
        app.login("user", "wrong-password12"),
        Err(AppError::Invalid(_))
    ));
}

#[test]
fn expired_session_is_refused() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    app.init_user("user", PASSWORD).expect("init");

    let generation = app
        .store
        .get_user()
        .expect("user")
        .expect("ligne")
        .credential_generation;
    let token = fs_ez_app::token::random_token().expect("token");
    let now = app.now();
    app.store
        .create_session(
            &fs_ez_app::token::hash_token(&token),
            generation,
            now - Duration::hours(1),
            now - Duration::seconds(1),
        )
        .expect("session");

    assert!(app.validate_session(&token).expect("validation").is_none());
}

#[test]
fn last_seen_is_not_rewritten_within_a_minute() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    app.init_user("user", PASSWORD).expect("init");
    let session = app.login("user", PASSWORD).expect("login");
    let hash = fs_ez_app::token::hash_token(&session.token);

    let before = app
        .store
        .find_session(&hash)
        .expect("session")
        .expect("ligne");
    std::thread::sleep(std::time::Duration::from_millis(20));
    app.validate_session(&session.token).expect("validation");
    let after = app
        .store
        .find_session(&hash)
        .expect("session")
        .expect("ligne");
    assert_eq!(before.last_seen_at, after.last_seen_at);
}

#[test]
fn purge_removes_only_expired_sessions() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    app.init_user("user", PASSWORD).expect("init");
    let live = app.login("user", PASSWORD).expect("login");

    let generation = app
        .store
        .get_user()
        .expect("user")
        .expect("ligne")
        .credential_generation;
    let expired = fs_ez_app::token::random_token().expect("token");
    let now = app.now();
    app.store
        .create_session(
            &fs_ez_app::token::hash_token(&expired),
            generation,
            now - Duration::hours(1),
            now - Duration::seconds(1),
        )
        .expect("session");

    assert_eq!(app.purge_expired_sessions().expect("purge"), 1);
    assert!(
        app.validate_session(&expired)
            .expect("validation")
            .is_none()
    );
    assert!(
        app.validate_session(&live.token)
            .expect("validation")
            .is_some()
    );
}

#[test]
fn reset_user_changes_password_and_revokes_everything() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    app.init_user("user", PASSWORD).expect("init");
    let session = app.login("user", PASSWORD).expect("login");
    let created = app.create_api_token("ci", None).expect("jeton");

    app.reset_user("user", "another-long-password")
        .expect("reset");

    assert!(!app.verify_user("user", PASSWORD).expect("vérification"));
    assert!(
        app.verify_user("user", "another-long-password")
            .expect("vérification")
    );
    assert!(
        app.validate_session(&session.token)
            .expect("validation")
            .is_none()
    );
    assert!(app.list_api_tokens().expect("liste").is_empty());
    assert!(
        app.validate_api_token(&created.token)
            .expect("validation")
            .is_none()
    );
}

// Login suspendu apres verification du hash : le reset commite avant create_session, la session doit etre refusee.
#[test]
fn session_from_pre_reset_generation_is_rejected() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    app.init_user("user", PASSWORD).expect("init");
    let stale = app
        .store
        .get_user()
        .expect("user")
        .expect("ligne")
        .credential_generation;

    app.reset_user("user", "another-long-password")
        .expect("reset");

    let token = fs_ez_app::token::random_token().expect("token");
    let now = app.now();
    app.store
        .create_session(
            &fs_ez_app::token::hash_token(&token),
            stale,
            now,
            now + Duration::days(1),
        )
        .expect("insertion toleree mais session perimee");

    assert!(app.validate_session(&token).expect("validation").is_none());
}

#[test]
fn api_token_create_validate_and_revoke() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    app.init_user("user", PASSWORD).expect("init");

    let generation = app
        .store
        .get_user()
        .expect("user")
        .expect("ligne")
        .credential_generation;
    let created = app
        .create_api_token("  ci deploy  ", None)
        .expect("création");
    assert_eq!(created.record.name, "ci deploy");
    assert_eq!(created.record.credential_generation, generation);
    assert!(created.record.last_used_at.is_none());
    assert!(fs_ez_app::token::is_valid_token_format(&created.token));

    let listed = app.list_api_tokens().expect("liste");
    assert_eq!(listed.len(), 1);
    assert_eq!(
        listed[0].token_hash,
        fs_ez_app::token::hash_token(&created.token)
    );

    let validated = app
        .validate_api_token(&created.token)
        .expect("validation")
        .expect("jeton valide");
    assert_eq!(validated.id, created.record.id);
    assert!(validated.last_used_at.is_some());

    assert!(app.revoke_api_token(created.record.id).expect("révocation"));
    assert!(!app.revoke_api_token(created.record.id).expect("révocation"));
    assert!(
        app.validate_api_token(&created.token)
            .expect("validation")
            .is_none()
    );
}

// Jeton cree avant le reset : sa generation ne correspond plus, il reste inerte.
#[test]
fn api_token_from_pre_reset_generation_is_rejected() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    app.init_user("user", PASSWORD).expect("init");
    let stale = app
        .store
        .get_user()
        .expect("user")
        .expect("ligne")
        .credential_generation;
    let created = app
        .create_api_token_for_generation("pre", None, stale)
        .expect("jeton pre-reset");

    app.reset_user("user", "another-long-password")
        .expect("reset");

    assert!(app.list_api_tokens().expect("liste").is_empty());
    assert!(
        app.validate_api_token(&created.token)
            .expect("validation")
            .is_none()
    );
}

#[test]
fn api_token_insert_refuses_a_stale_generation() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    app.init_user("user", PASSWORD).expect("init");
    let stale = app
        .store
        .get_user()
        .expect("user")
        .expect("ligne")
        .credential_generation;

    app.reset_user("user", "another-long-password")
        .expect("reset");
    let current = app
        .store
        .get_user()
        .expect("user")
        .expect("ligne")
        .credential_generation;
    assert_eq!(current, stale + 1);

    assert!(matches!(
        app.create_api_token_for_generation("stale", None, stale),
        Err(AppError::Store(fs_ez_store::StoreError::Conflict(_)))
    ));
    assert!(app.list_api_tokens().expect("liste").is_empty());

    let created = app
        .create_api_token_for_generation("after", None, current)
        .expect("jeton post-reset");
    assert_eq!(created.record.credential_generation, current);
    assert!(
        app.validate_api_token(&created.token)
            .expect("validation")
            .is_some()
    );
}

// Ligne inseree hors du store apres un reset : la validation compare la generation et l'ignore.
#[test]
fn api_token_with_stale_generation_is_inert() {
    let temp = TempDir::new().expect("tempdir");
    let path = temp.path().join("config").join("fs-ez.db");
    let token = fs_ez_app::token::random_token().expect("token");
    {
        let conn = db::open(&path).expect("base");
        conn.execute(
            "INSERT INTO user(id, username, password_hash, created_at, updated_at, credential_generation)
             VALUES (1, 'user', 'hash', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', 1)",
            [],
        )
        .expect("user");
        conn.execute(
            "INSERT INTO api_token(name, token_hash, created_at, expires_at, credential_generation)
             VALUES ('stale', ?1, '2026-01-01T00:00:00Z', NULL, 0)",
            [fs_ez_app::token::hash_token(&token)],
        )
        .expect("jeton");
    }
    let store = Arc::new(SqliteStore::open(&path).expect("store"));
    let app = app_with_store(&temp, store);

    assert!(
        app.validate_api_token(&token)
            .expect("validation")
            .is_none()
    );
}

#[test]
fn api_token_validation_and_expiration() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    app.init_user("user", PASSWORD).expect("init");

    assert!(matches!(
        app.create_api_token("   ", None),
        Err(AppError::Invalid(_))
    ));
    assert!(matches!(
        app.create_api_token(&"x".repeat(101), None),
        Err(AppError::Invalid(_))
    ));
    assert!(matches!(
        app.create_api_token("past", Some(app.now() - Duration::seconds(1))),
        Err(AppError::Invalid(_))
    ));

    let future = app
        .create_api_token("future", Some(app.now() + Duration::days(1)))
        .expect("création");
    assert!(
        app.validate_api_token(&future.token)
            .expect("validation")
            .is_some()
    );

    assert!(matches!(
        app.create_api_token("future", None),
        Err(AppError::Store(fs_ez_store::StoreError::Conflict(_)))
    ));

    let generation = app
        .store
        .get_user()
        .expect("user")
        .expect("ligne")
        .credential_generation;
    let stale = fs_ez_app::token::random_token().expect("token");
    let now = app.now();
    app.store
        .create_api_token(
            "stale",
            &fs_ez_app::token::hash_token(&stale),
            generation,
            now - Duration::hours(1),
            Some(now - Duration::seconds(1)),
        )
        .expect("jeton");
    assert!(
        app.validate_api_token(&stale)
            .expect("validation")
            .is_none()
    );
    let stored = app
        .store
        .find_api_token_by_hash(&fs_ez_app::token::hash_token(&stale))
        .expect("lookup")
        .expect("ligne");
    assert!(stored.last_used_at.is_none());
}

#[test]
fn settings_get_set_and_validate() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);

    assert_eq!(app.get_setting("ui.theme").expect("lecture"), None);
    assert_eq!(app.all_settings().expect("liste"), Vec::new());

    app.set_setting("ui.theme", "dark").expect("écriture");
    assert_eq!(
        app.get_setting("ui.theme").expect("lecture"),
        Some("dark".to_string())
    );
    assert_eq!(
        app.settings().expect("réglages").theme,
        fs_ez_core::Theme::Dark
    );

    assert!(app.set_setting("ui.theme", "neon").is_err());
    assert!(app.set_setting("ui.unknown", "x").is_err());
    assert_eq!(
        app.get_setting("ui.theme").expect("lecture"),
        Some("dark".to_string())
    );

    app.set_setting("ui.page_size", "250").expect("écriture");
    let all = app.all_settings().expect("liste");
    assert_eq!(all.len(), 2);
    assert!(all.iter().any(|(key, _)| key == "ui.page_size"));
}
