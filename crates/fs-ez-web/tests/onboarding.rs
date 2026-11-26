//! Onboarding : premier compte utilisateur choisi depuis le web.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Method, Request, StatusCode, header};
use axum::response::Response;
use fs_ez_app::App;
use fs_ez_fingerprint::Blake3Fingerprinter;
use fs_ez_fs::{Root, SystemFs};
use fs_ez_sqlite::{SqliteStore, db};
use fs_ez_store::Store;
use fs_ez_web::backend::{AppBackend, Backend};
use fs_ez_web::config::WebConfig;
use fs_ez_web::locales::Locales;
use fs_ez_web::router::build_router;
use fs_ez_web::state::AppState;
use tempfile::TempDir;
use tower::ServiceExt;

const NEW_USERNAME: &str = "alice";
const NEW_PASSWORD: &str = "correct-horse-battery";

fn state() -> (AppState<AppBackend>, TempDir) {
    let dir = TempDir::new().expect("tempdir");
    let db_path = dir.path().join("config/fs-ez.db");
    let store: Arc<dyn Store> = Arc::new(SqliteStore::from_connection(
        db::open_in_memory().expect("base memoire"),
    ));
    let app = Arc::new(App::new(
        store,
        Arc::new(SystemFs),
        Arc::new(Blake3Fingerprinter::default()),
        Root::discover(Some(dir.path())).expect("root"),
    ));
    let config = WebConfig {
        host: "127.0.0.1".to_string(),
        port: 8678,
        max_upload: 1024,
        root: dir.path().to_path_buf(),
        db_path,
        files: None,
    };
    let state = AppState::new(
        Arc::new(AppBackend::new(Arc::clone(&app))),
        Arc::new(config),
        Arc::new(Locales::load().expect("locales")),
    );
    (state, dir)
}

fn request(method: Method, uri: &str, body: &str) -> Request<Body> {
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 4242))))
        .body(Body::from(body.to_string()))
        .expect("requete");
    request.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/x-www-form-urlencoded"),
    );
    request
}

async fn send(router: &Router, request: Request<Body>) -> Response<Body> {
    router.clone().oneshot(request).await.expect("reponse")
}

async fn body(response: Response<Body>) -> String {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("corps");
    String::from_utf8_lossy(&bytes).into_owned()
}

fn location(response: &Response<Body>) -> String {
    response
        .headers()
        .get(header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string()
}

fn set_cookie(response: &Response<Body>) -> String {
    response
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string()
}

fn session_token(cookie: &str) -> String {
    cookie
        .split(';')
        .next()
        .and_then(|pair| pair.trim().strip_prefix("fs-ez_session="))
        .unwrap_or_default()
        .to_string()
}

#[tokio::test]
async fn pages_redirect_to_onboarding_until_an_user_exists() {
    let (state, _dir) = state();
    let router = build_router(state.clone());

    let response = send(&router, request(Method::GET, "/", "")).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&response), "/setup");

    let response = send(&router, request(Method::GET, "/login", "")).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&response), "/setup");

    let response = send(&router, request(Method::GET, "/setup", "")).await;
    assert_eq!(response.status(), StatusCode::OK);
    let html = body(response).await;
    assert!(html.contains("Welcome to fs-ez"), "{html}");
    assert!(html.contains("action=\"/setup\""), "{html}");
    assert!(html.contains("name=\"username\""), "{html}");
    assert!(html.contains("minlength=\"12\""), "{html}");
}

#[tokio::test]
async fn second_setup_does_not_replace_the_first_account() {
    let (state, _dir) = state();
    let router = build_router(state.clone());

    let response = send(
        &router,
        request(
            Method::POST,
            "/setup",
            &format!("username={NEW_USERNAME}&password={NEW_PASSWORD}"),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let cookie = session_token(&set_cookie(&response));
    assert!(!cookie.is_empty());
    let first = state
        .backend
        .app()
        .store
        .get_user()
        .expect("user")
        .expect("present");

    let response = send(
        &router,
        request(
            Method::POST,
            "/setup",
            "username=bravo&password=bravo-password-123",
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&response), "/login");

    let unchanged = state
        .backend
        .app()
        .store
        .get_user()
        .expect("user")
        .expect("present");
    assert_eq!(unchanged.id, first.id);
    assert_eq!(unchanged.username, first.username);
    assert_eq!(unchanged.password_hash, first.password_hash);

    let mut authenticated = request(Method::GET, "/api/session", "");
    authenticated.headers_mut().insert(
        header::COOKIE,
        header::HeaderValue::from_str(&format!("fs-ez_session={cookie}")).expect("cookie"),
    );
    let response = send(&router, authenticated).await;
    assert_eq!(response.status(), StatusCode::OK);
    let json: serde_json::Value = serde_json::from_str(&body(response).await).expect("json");
    assert_eq!(json["username"], NEW_USERNAME);

    let app = state.backend.app();
    assert!(
        app.verify_user(NEW_USERNAME, NEW_PASSWORD)
            .expect("verification")
    );
    assert!(
        !app.verify_user("bravo", "bravo-password-123")
            .expect("verification")
    );
}

#[tokio::test]
async fn setup_creates_the_chosen_user_and_opens_a_session() {
    let (state, _dir) = state();
    let router = build_router(state.clone());

    let response = send(
        &router,
        request(
            Method::POST,
            "/setup",
            &format!("username={NEW_USERNAME}&password=short"),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let html = body(response).await;
    assert!(html.contains("at least 12 characters"), "{html}");
    assert!(!state.backend.app().has_user().expect("has_user"));

    let response = send(
        &router,
        request(
            Method::POST,
            "/setup",
            &format!("username={NEW_USERNAME}&password={NEW_PASSWORD}"),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&response), "/");
    let cookie = session_token(&set_cookie(&response));
    assert!(!cookie.is_empty());

    let user = state
        .backend
        .app()
        .store
        .get_user()
        .expect("user")
        .expect("present");
    assert_eq!(user.username, NEW_USERNAME);
    assert!(user.password_hash.starts_with("$argon2"));
    assert!(!user.password_hash.contains(NEW_PASSWORD));

    let mut authenticated = request(Method::GET, "/", "");
    authenticated.headers_mut().insert(
        header::COOKIE,
        header::HeaderValue::from_str(&format!("fs-ez_session={cookie}")).expect("cookie"),
    );
    let response = send(&router, authenticated).await;
    assert_eq!(response.status(), StatusCode::OK);

    let response = send(&router, request(Method::GET, "/setup", "")).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&response), "/login");
}

#[tokio::test]
async fn five_invalid_forms_do_not_lock_out_a_valid_setup() {
    let (state, _dir) = state();
    let router = build_router(state.clone());

    for _ in 0..5 {
        let response = send(
            &router,
            request(
                Method::POST,
                "/setup",
                &format!("username={NEW_USERNAME}&password=short"),
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert!(!state.backend.app().has_user().expect("has_user"));
    }

    let response = send(
        &router,
        request(
            Method::POST,
            "/setup",
            &format!("username={NEW_USERNAME}&password={NEW_PASSWORD}"),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&response), "/");
    assert!(!session_token(&set_cookie(&response)).is_empty());

    let user = state
        .backend
        .app()
        .store
        .get_user()
        .expect("user")
        .expect("present");
    assert_eq!(user.username, NEW_USERNAME);
}

#[tokio::test]
async fn setup_returns_429_while_the_hash_gate_is_held() {
    let (state, _dir) = state();
    let router = build_router(state.clone());
    let held = state
        .setup_gate
        .clone()
        .try_acquire_owned()
        .expect("créneau libre");

    let response = send(
        &router,
        request(
            Method::POST,
            "/setup",
            &format!("username={NEW_USERNAME}&password={NEW_PASSWORD}"),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        response
            .headers()
            .get(header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok()),
        Some("1")
    );
    assert!(!state.backend.app().has_user().expect("has_user"));

    drop(held);
    let response = send(
        &router,
        request(
            Method::POST,
            "/setup",
            &format!("username={NEW_USERNAME}&password={NEW_PASSWORD}"),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(location(&response), "/");
    assert!(!session_token(&set_cookie(&response)).is_empty());
}

#[tokio::test]
async fn concurrent_setups_create_one_account_and_one_session() {
    let (state, _dir) = state();
    let router = build_router(state.clone());
    let attempt = |n: usize| {
        request(
            Method::POST,
            "/setup",
            &format!("username=user{n}&password=password-{n}-abcdef"),
        )
    };

    // `join!` interroge chaque futur à tour de rôle : un seul détient le créneau pendant le hachage.
    let (r1, r2, r3, r4, r5, r6, r7, r8) = tokio::join!(
        send(&router, attempt(1)),
        send(&router, attempt(2)),
        send(&router, attempt(3)),
        send(&router, attempt(4)),
        send(&router, attempt(5)),
        send(&router, attempt(6)),
        send(&router, attempt(7)),
        send(&router, attempt(8)),
    );
    let responses = [r1, r2, r3, r4, r5, r6, r7, r8];
    let mut created = 0;
    let mut sessions = 0;
    for response in &responses {
        match response.status() {
            StatusCode::SEE_OTHER => {
                assert_eq!(location(response), "/");
                created += 1;
                if !session_token(&set_cookie(response)).is_empty() {
                    sessions += 1;
                }
            }
            StatusCode::CONFLICT | StatusCode::TOO_MANY_REQUESTS => {}
            other => panic!("statut inattendu: {other}"),
        }
    }
    assert_eq!(created, 1);
    assert_eq!(sessions, 1);

    let user = state
        .backend
        .app()
        .store
        .get_user()
        .expect("user")
        .expect("present");
    assert_eq!(user.username, "user1");
}
