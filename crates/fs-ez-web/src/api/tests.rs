//! Tests de routeur de l'API JSON (fiche 40).

mod auth;
mod dto;
mod errors;
mod events;
mod files;
mod groups;
mod status;
mod tags;
mod tokens;
mod views;

use std::net::SocketAddr;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Method, Request, Response, StatusCode, header};
use fs_ez_app::EnsureOptions;
use fs_ez_core::FileId;
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

use crate::backend::{AppBackend, Backend};
use crate::router::build_router;
use crate::state::AppState;
use crate::test_support::{PASSWORD, USERNAME, state};

fn seed_file(state: &AppState<AppBackend>, temp: &TempDir, name: &str) -> FileId {
    let path = temp.path().join(name);
    std::fs::write(&path, format!("contenu secret {name}")).expect("fichier");
    state
        .backend
        .app()
        .ensure_file(&path, EnsureOptions { force: false })
        .expect("fichier enregistré")
        .id
}

fn json_request(
    method: Method,
    uri: &str,
    body: Option<&Value>,
    cookie: Option<&str>,
    bearer: Option<&str>,
    site: Option<&'static str>,
) -> Request<Body> {
    let payload = body.map(|value| value.to_string());
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 4242))));
    if let Some(cookie) = cookie {
        builder = builder.header(header::COOKIE, format!("fs-ez_session={cookie}"));
    }
    if let Some(bearer) = bearer {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {bearer}"));
    }
    if let Some(site) = site {
        builder = builder.header("sec-fetch-site", site);
    }
    if payload.is_some() {
        builder = builder.header(header::CONTENT_TYPE, "application/json");
    }
    builder
        .body(payload.map_or_else(Body::empty, Body::from))
        .expect("requête")
}

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    json: Value,
    text: String,
}

async fn send(router: &Router, request: Request<Body>) -> Reply {
    let response: Response<Body> = router.clone().oneshot(request).await.expect("réponse");
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("corps");
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let json = serde_json::from_str(&text).unwrap_or(Value::Null);
    Reply {
        status,
        headers,
        json,
        text,
    }
}

async fn call(
    router: &Router,
    method: Method,
    uri: &str,
    body: Option<&Value>,
    cookie: Option<&str>,
) -> Reply {
    send(router, json_request(method, uri, body, cookie, None, None)).await
}

async fn login(router: &Router) -> String {
    let body = format!("username={USERNAME}&password={PASSWORD}");
    let request = Request::builder()
        .method(Method::POST)
        .uri("/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 4242))))
        .body(Body::from(body))
        .expect("requête");
    let reply = send(router, request).await;
    assert_eq!(reply.status, StatusCode::SEE_OTHER);
    let cookie = reply
        .headers
        .get(header::SET_COOKIE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    cookie
        .split(';')
        .next()
        .and_then(|pair| pair.trim().strip_prefix("fs-ez_session="))
        .unwrap_or_default()
        .to_string()
}

async fn create_tag(router: &Router, cookie: &str, name: &str) {
    let reply = call(
        router,
        Method::POST,
        "/api/tags",
        Some(&json!({ "name": name })),
        Some(cookie),
    )
    .await;
    assert_eq!(reply.status, StatusCode::CREATED, "{}", reply.text);
}

fn assert_error(reply: &Reply, status: StatusCode, code: &str) {
    assert_eq!(reply.status, status, "{}", reply.text);
    assert_eq!(reply.json["schema"], 1);
    assert_eq!(reply.json["code"], code, "{}", reply.text);
    assert!(reply.json["error"].is_string());
}

fn queue_file(state: &AppState<AppBackend>, temp: &TempDir, name: &str) -> i64 {
    let path = temp.path().join(name);
    std::fs::write(&path, format!("contenu {name}")).expect("fichier importe");
    match state.backend.app().import_path(&path).expect("import") {
        fs_ez_app::LibraryOutcome::Indexed(file) => file.id.0,
        other => panic!("import inattendu: {other:?}"),
    }
}
