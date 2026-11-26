//! Assets statiques : disque d'abord, fichiers pré-gzippés embarqués en secours.

use std::path::{Path as FsPath, PathBuf};
use std::time::UNIX_EPOCH;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use tokio::fs;

use crate::assets::PRECOMPRESSED_ASSETS;
use crate::backend::Backend;
use crate::state::AppState;

const DEV_CACHE: &str = "public, max-age=3600";
const IMMUTABLE_CACHE: &str = "public, max-age=31536000, immutable";

pub async fn static_file<B: Backend>(
    method: Method,
    State(_state): State<AppState<B>>,
    Path(raw): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some(path) = normalize(&raw) else {
        return not_found();
    };
    let if_none_match = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok());

    if let Some(asset) = read_disk(&path).await {
        return asset_response(&method, asset, if_none_match);
    }
    if let Some(asset) = embedded(&path) {
        return embedded_response(&method, asset, if_none_match);
    }
    not_found()
}

struct DiskAsset {
    path: String,
    bytes: Vec<u8>,
    etag: String,
}

async fn read_disk(path: &str) -> Option<DiskAsset> {
    let root = static_root()?.join("static");
    let base = fs::canonicalize(&root).await.ok()?;
    let full = fs::canonicalize(base.join(path)).await.ok()?;
    if !full.starts_with(&base) {
        return None;
    }
    let meta = fs::metadata(&full).await.ok()?;
    if !meta.is_file() {
        return None;
    }
    let bytes = fs::read(&full).await.ok()?;
    Some(DiskAsset {
        path: path.to_string(),
        etag: disk_etag(&meta),
        bytes,
    })
}

fn disk_etag(meta: &std::fs::Metadata) -> String {
    let nanos = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |elapsed| elapsed.as_nanos());
    format!("\"{:x}-{:x}\"", meta.len(), nanos)
}

fn asset_response(method: &Method, asset: DiskAsset, if_none_match: Option<&str>) -> Response {
    build_asset_response(
        method,
        &asset.path,
        asset.bytes,
        &asset.etag,
        None,
        None,
        if_none_match,
    )
}

fn embedded_response(
    method: &Method,
    asset: &crate::assets::PrecompressedAsset,
    if_none_match: Option<&str>,
) -> Response {
    let etag = format!("\"gz-{}-{}\"", asset.path, asset.data.len());
    build_asset_response(
        method,
        asset.path,
        asset.data,
        &etag,
        Some("gzip"),
        Some("Accept-Encoding"),
        if_none_match,
    )
}

fn build_asset_response(
    method: &Method,
    path: &str,
    bytes: impl Into<Body>,
    etag: &str,
    encoding: Option<&'static str>,
    vary: Option<&'static str>,
    if_none_match: Option<&str>,
) -> Response {
    let cache = cache_control(path);
    if matches_etag(if_none_match, etag) {
        return not_modified(etag, cache);
    }
    let body = if *method == Method::HEAD {
        Body::empty()
    } else {
        bytes.into()
    };
    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type(path));
    if let Some(encoding) = encoding {
        builder = builder.header(header::CONTENT_ENCODING, encoding);
    }
    if let Some(vary) = vary {
        builder = builder.header(header::VARY, vary);
    }
    match builder
        .header(header::CACHE_CONTROL, cache)
        .header(header::ETAG, etag)
        .body(body)
    {
        Ok(response) => response,
        Err(_) => not_found(),
    }
}

fn embedded(path: &str) -> Option<&'static crate::assets::PrecompressedAsset> {
    PRECOMPRESSED_ASSETS.iter().find(|asset| asset.path == path)
}

fn not_modified(etag: &str, cache: &str) -> Response {
    match Response::builder()
        .status(StatusCode::NOT_MODIFIED)
        .header(header::ETAG, etag)
        .header(header::CACHE_CONTROL, cache)
        .body(Body::empty())
    {
        Ok(response) => response,
        Err(_) => not_found(),
    }
}

fn matches_etag(header_value: Option<&str>, etag: &str) -> bool {
    header_value.is_some_and(|value| {
        value
            .split(',')
            .map(|candidate| candidate.trim().trim_start_matches("W/"))
            .any(|candidate| candidate == etag)
    })
}

fn cache_control(path: &str) -> &'static str {
    if path.starts_with("vendor/") {
        IMMUTABLE_CACHE
    } else {
        DEV_CACHE
    }
}

fn content_type(path: &str) -> &'static str {
    match FsPath::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
    {
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("woff2") => "font/woff2",
        Some("ico") => "image/x-icon",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}

/// Refuse `..`, les chemins absolus, le NUL et les segments vides implicites.
fn normalize(raw: &str) -> Option<String> {
    if raw.contains('\0') || raw.contains('\\') || raw.starts_with('/') {
        return None;
    }
    let mut parts = Vec::new();
    for part in raw.split('/') {
        match part {
            "" | "." => {}
            ".." => return None,
            _ => parts.push(part),
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("/"))
}

/// Remonte depuis le CWD : binaire et tests n'ont pas le même répertoire courant.
fn static_root() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        if dir.join("static").is_dir() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn not_found() -> Response {
    (StatusCode::NOT_FOUND, "not found").into_response()
}

#[cfg(test)]
mod tests;
