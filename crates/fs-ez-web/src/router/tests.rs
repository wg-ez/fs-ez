use std::net::SocketAddr;

use axum::body::{Body, to_bytes};
use axum::extract::ConnectInfo;
use axum::http::{Method, Request, Response, StatusCode, header};
use serde_json::Value;
use tempfile::TempDir;
use tower::ServiceExt;

use super::*;
use crate::test_support::{PASSWORD, USERNAME, state};

fn request(method: Method, uri: &str, body: Body) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 4242))))
        .body(body)
        .expect("requête")
}

fn get(uri: &str) -> Request<Body> {
    request(Method::GET, uri, Body::empty())
}

fn post_form(uri: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 4242))))
        .body(Body::from(body.to_string()))
        .expect("requête")
}

fn with_cookie(mut req: Request<Body>, token: &str) -> Request<Body> {
    req.headers_mut().insert(
        header::COOKIE,
        header::HeaderValue::from_str(&format!("fs-ez_session={token}")).expect("cookie"),
    );
    req
}

fn with_site(mut req: Request<Body>, site: &'static str) -> Request<Body> {
    req.headers_mut()
        .insert("sec-fetch-site", header::HeaderValue::from_static(site));
    req
}

fn with_origin(mut req: Request<Body>, origin: &'static str) -> Request<Body> {
    req.headers_mut()
        .insert(header::ORIGIN, header::HeaderValue::from_static(origin));
    req
}

async fn send(router: &Router, req: Request<Body>) -> Response<Body> {
    router.clone().oneshot(req).await.expect("réponse")
}

async fn body_text(response: Response<Body>) -> String {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("corps");
    String::from_utf8_lossy(&bytes).into_owned()
}

fn set_cookie(response: &Response<Body>) -> String {
    response
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string()
}

fn session_token(response: &Response<Body>) -> String {
    set_cookie(response)
        .split(';')
        .next()
        .and_then(|pair| pair.trim().strip_prefix("fs-ez_session="))
        .unwrap_or_default()
        .to_string()
}

fn location(response: &Response<Body>) -> String {
    response
        .headers()
        .get(header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string()
}

fn login_body(password: &str) -> String {
    format!("username={USERNAME}&password={password}")
}

async fn login(router: &Router, next: Option<&str>) -> Response<Body> {
    let mut body = login_body(PASSWORD);
    if let Some(next) = next {
        let encoded: String = next.bytes().map(|byte| format!("%{byte:02X}")).collect();
        body.push_str(&format!("&next={encoded}"));
    }
    send(router, post_form("/login", &body)).await
}

#[path = "tests/auth.rs"]
mod auth;
#[path = "tests/health.rs"]
mod health;
#[path = "tests/login.rs"]
mod login;
#[path = "tests/security.rs"]
mod security;
#[path = "tests/static_assets.rs"]
mod static_assets;
#[path = "tests/templates/mod.rs"]
mod templates;
#[path = "tests/themes.rs"]
mod themes;
