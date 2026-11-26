//! Logging DAV : une ligne par requête et événements clés, jamais d'identifiant secret.

use std::time::Instant;

use axum::extract::Request;
use axum::http::{HeaderMap, header};
use axum::middleware::Next;
use axum::response::Response;
use tracing::{debug, info, warn};

pub const TARGET: &str = "fs_ez_web::dav";
const MAX_LOG_FIELD: usize = 240;
const DEPTH: header::HeaderName = header::HeaderName::from_static("depth");
const DESTINATION: header::HeaderName = header::HeaderName::from_static("destination");

pub async fn request_log(req: Request, next: Next) -> Response {
    let started = Instant::now();
    let method = req.method().as_str().to_string();
    let path = clean(req.uri().path());
    let query = clean(req.uri().query().unwrap_or_default());
    let user_agent = header(req.headers(), header::USER_AGENT);
    let depth = header(req.headers(), DEPTH);
    let range = header(req.headers(), header::RANGE);
    let destination = header(req.headers(), DESTINATION);
    let incoming = content_length(req.headers());
    debug!(
        target: TARGET,
        method = %method,
        path = %path,
        query = %query,
        headers = %diagnostic_headers(req.headers()),
        "dav request headers"
    );
    let response = next.run(req).await;
    let status = response.status();
    let outgoing = content_length(response.headers());
    debug!(
        target: TARGET,
        method = %method,
        path = %path,
        status = status.as_u16(),
        headers = %diagnostic_headers(response.headers()),
        "dav response headers"
    );
    info!(
        target: TARGET,
        method = %method,
        path = %path,
        status = status.as_u16(),
        duration_ms = started.elapsed().as_millis() as u64,
        user_agent = user_agent.unwrap_or_default(),
        depth = depth.unwrap_or_default(),
        range = range.unwrap_or_default(),
        destination = destination.unwrap_or_default(),
        content_length = incoming,
        response_length = outgoing,
        "dav request"
    );
    response
}

/// En-têtes utiles au diagnostic, secrets et jetons de verrou masqués.
fn diagnostic_headers(headers: &HeaderMap) -> String {
    const WANTED: &[&str] = &[
        "content-type",
        "content-length",
        "content-range",
        "accept-ranges",
        "depth",
        "range",
        "if",
        "if-range",
        "if-match",
        "if-none-match",
        "if-modified-since",
        "if-unmodified-since",
        "destination",
        "overwrite",
        "lock-token",
        "timeout",
        "translate",
        "user-agent",
        "etag",
        "last-modified",
        "dav",
        "allow",
        "www-authenticate",
    ];
    let mut parts = Vec::new();
    for name in WANTED {
        for value in headers.get_all(*name) {
            let Ok(value) = value.to_str() else {
                continue;
            };
            let value = if matches!(*name, "if" | "lock-token") {
                mask_tokens(value)
            } else {
                clean(value)
            };
            parts.push(format!("{name}={value}"));
        }
    }
    parts.join(" | ")
}

fn mask_tokens(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut inside = false;
    for character in raw.chars() {
        match character {
            '<' => {
                inside = true;
                out.push('<');
            }
            '>' => {
                inside = false;
                out.push('>');
            }
            _ if inside => {}
            _ => out.push(character),
        }
    }
    // Une valeur sans chevrons reste lisible, seul le contenu opaque est masqué.
    if out.contains('<') { out } else { clean(raw) }
}

pub fn auth_ok(token_id: i64, token_name: &str) {
    info!(
        target: TARGET,
        token_id,
        token_name = %clean(token_name),
        "dav auth ok"
    );
}

pub fn auth_failed(reason: &'static str) {
    warn!(target: TARGET, reason, "dav auth failed");
}

fn header(headers: &HeaderMap, name: header::HeaderName) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(clean)
}

fn content_length(headers: &HeaderMap) -> u64 {
    headers
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

fn clean(raw: &str) -> String {
    raw.chars()
        .take(MAX_LOG_FIELD)
        .map(|character| {
            if character.is_control() {
                '_'
            } else {
                character
            }
        })
        .collect()
}
