//! Adaptateur WebDAV sous `/dav`, authentifié par jeton d'API Basic.
//!
//! Namespace virtuel calculé depuis SQLite : `tags/{tag}/{tag}.../All Files/`, `tags/All Empty/` et `tags/{tag}.../All Empty/` pour les tags sans fichier, fichiers directs au fond de sélection, `views/{groupe}/{vue}/`, vues système `untagged/`, `recently tagged/`, `recent files/`, `inbox/`, alias `{nom}`.
//! PUT remplace un alias via `replace_file_contents`, importe et tague un fichier sous une collection de tags, ou importe dans un dossier virtuel.
//! MOVE promeut un temporaire, renomme ou déplace un dossier virtuel, renomme un alias, ou remplace le contenu d'un alias par celui d'un autre.
//! DELETE supprime un alias et son fichier, refuse une collection, seuls les temporaires DAV et les alias sont supprimables.
//! DELETE dans l'inbox reste interdit, un alias ne supprime jamais son fichier pendant un MOVE.
//! PROPFIND Depth 1 lit les collections par pages SQL de 500, sans liste complète en mémoire.
//! Le protocole XML, Range, conditionnelles et LOCK/UNLOCK sont délégués à `dav-server`.

pub mod auth;
pub(crate) mod backend;
mod cache;
pub(crate) mod locks;
pub(crate) mod logging;
pub(crate) mod path;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use dav_server::davpath::DavPath;
use dav_server::{DavHandler, DavMethod, DavMethodSet};
use fs_ez_app::App;
use fs_ez_store::ApiTokenRecord;
use percent_encoding::percent_decode_str;
use time::OffsetDateTime;

use crate::backend::{Backend, blocking_join};
use crate::state::AppState;

use backend::{DavCredentials, DavFs};
use cache::TtlMap;

/// Révocation effective en quelques secondes, `touch_api_token` groupé sur la fenêtre.
const TOKEN_CACHE_TTL: Duration = Duration::from_secs(3);
/// Un seul utilisateur, quelques jetons actifs suffisent.
const TOKEN_CACHE_MAX: usize = 16;

/// Cache court des jetons d'API validés, porté par le service DAV.
#[derive(Clone, Debug)]
pub struct TokenCache {
    entries: TtlMap<Option<ApiTokenRecord>>,
}

impl Default for TokenCache {
    fn default() -> Self {
        Self {
            entries: TtlMap::new(TOKEN_CACHE_TTL, TOKEN_CACHE_MAX),
        }
    }
}

impl TokenCache {
    /// `Some(None)` = jeton refusé récemment, `None` = pas d'entrée fraîche.
    pub fn get(&self, token: &str) -> Option<Option<ApiTokenRecord>> {
        self.entries.get(&fs_ez_app::token::hash_token(token))
    }

    pub fn put(&self, token: &str, record: Option<ApiTokenRecord>) {
        self.entries
            .insert(fs_ez_app::token::hash_token(token), record);
    }

    pub fn remove(&self, token: &str) {
        self.entries.remove(&fs_ez_app::token::hash_token(token));
    }
}

#[derive(Clone)]
pub struct DavService {
    handler: DavHandler<DavCredentials>,
    max_upload: u64,
    tokens: TokenCache,
}

impl DavService {
    pub fn tokens(&self) -> &TokenCache {
        &self.tokens
    }
}

pub fn service(
    app: Arc<App>,
    max_upload: u64,
    events: tokio::sync::broadcast::Sender<crate::state::Event>,
) -> DavService {
    let mut methods = DavMethodSet::all();
    for method in [
        DavMethod::Patch,
        DavMethod::Copy,
        DavMethod::Report,
        DavMethod::MkCalendar,
        DavMethod::MkAddressbook,
    ] {
        methods.remove(method);
    }
    let handler = DavHandler::builder()
        .filesystem(Box::new(DavFs::new(app, max_upload, events)))
        .locksystem(locks::ExpiringLs::new())
        .strip_prefix("/dav")
        .methods(methods)
        .principal("fs-ez")
        .build_handler();
    DavService {
        handler,
        max_upload,
        tokens: TokenCache::default(),
    }
}

pub fn routes<B: Backend>() -> Router<AppState<B>> {
    Router::new()
        .route("/dav", any(dispatch::<B>))
        .route("/dav/", any(dispatch::<B>))
        .route("/dav/{*path}", any(dispatch::<B>))
}

async fn dispatch<B: Backend>(State(state): State<AppState<B>>, req: Request) -> Response {
    if req.method() == Method::DELETE
        && let Some(components) = request_path_components(req.uri().path())
        && !path::is_virtual_folder_target(&components)
        && targets_collection(state.backend.app(), components).await
    {
        tracing::warn!(
            target: logging::TARGET,
            path = %req.uri().path(),
            "dav collection delete refused"
        );
        return StatusCode::FORBIDDEN.into_response();
    }
    let move_paths = if is_move(&req) {
        request_path_components(req.uri().path()).zip(destination_components(&req))
    } else {
        None
    };
    if let Some((source, destination)) = move_paths {
        if let Some(status) =
            virtual_folder_move_status(state.backend.app(), source, destination.clone()).await
        {
            tracing::warn!(
                target: logging::TARGET,
                path = %req.uri().path(),
                status = status.as_u16(),
                "dav virtual folder move refused"
            );
            return status.into_response();
        }
        // dav-server supprime la destination (Depth infinity) avant le rename, un dossier existant perdrait ses fichiers.
        if targets_collection(state.backend.app(), destination).await {
            tracing::warn!(
                target: logging::TARGET,
                path = %req.uri().path(),
                "dav move onto existing collection refused"
            );
            return StatusCode::CONFLICT.into_response();
        }
    }
    handle(&state.dav, req).await
}

/// Segments de la requête, calculés avant tout `await` car `DavPath` n'est pas `Send`.
fn request_path_components(raw: &str) -> Option<Vec<String>> {
    let mut path = DavPath::new(raw).ok()?;
    path.set_prefix("/dav").ok()?;
    path::parse_components(path.as_bytes()).ok()
}

fn destination_components(req: &Request) -> Option<Vec<String>> {
    let uri = destination_uri(req)?;
    request_path_components(uri.path())
}

/// En-tête `Destination` de MOVE, absent ou mal formé rendu `None`.
fn destination_uri(req: &Request) -> Option<axum::http::Uri> {
    req.headers()
        .get("destination")?
        .to_str()
        .ok()?
        .parse()
        .ok()
}

fn is_move(req: &Request) -> bool {
    req.method().as_str() == "MOVE"
}

/// MOVE d'un dossier virtuel : destination absente d'un parent existant uniquement.
async fn virtual_folder_move_status(
    app: Arc<App>,
    source: Vec<String>,
    destination: Vec<String>,
) -> Option<StatusCode> {
    if source == destination {
        return None;
    }
    let resolved = blocking_join(Arc::clone(&app), move |app| path::resolve(app, &source)).await;
    let Ok(Ok(path::Node::Collection(collection))) = resolved else {
        return None;
    };
    if !matches!(collection.kind, path::CollectionKind::VirtualFolder { .. }) {
        return None;
    }
    let resolved = blocking_join(app, move |app| path::resolve(app, &destination)).await;
    match resolved {
        Ok(Ok(path::Node::Unknown { collection, .. }))
            if matches!(
                collection.kind,
                path::CollectionKind::VirtualFolders | path::CollectionKind::VirtualFolder { .. }
            ) =>
        {
            None
        }
        _ => Some(StatusCode::CONFLICT),
    }
}

/// DELETE d'une collection refusé avant `dav-server`, qui supprimerait sinon chaque alias enfant.
async fn targets_collection(app: Arc<App>, components: Vec<String>) -> bool {
    if path::structure_is_file(&components) {
        return false;
    }
    let resolved = blocking_join(app, move |app| path::resolve(app, &components)).await;
    matches!(resolved, Ok(Ok(path::Node::Collection(_))))
}

async fn handle(service: &DavService, mut req: Request) -> Response {
    let method = req.method().clone();
    if method == Method::HEAD {
        // HEAD ignore Range comme avant la migration, dav-server appliquerait la plage.
        req.headers_mut().remove(header::RANGE);
        req.headers_mut().remove(header::IF_RANGE);
    }
    if let Err(status) = prevalidate(&req, service.max_upload) {
        tracing::warn!(
            target: logging::TARGET,
            method = %method,
            path = %req.uri().path(),
            status = status.as_u16(),
            "dav request rejected before handler"
        );
        return status.into_response();
    }
    let credentials = DavCredentials {
        allow_alias_remove: is_move(&req),
        allow_folder_tree_delete: method == Method::DELETE,
        mtime: request_mtime(&req),
    };
    let response = service
        .handler
        .handle_guarded(req, "fs-ez".to_string(), credentials)
        .await;
    let mut response = response.map(Body::new).into_response();
    if method == Method::OPTIONS {
        response
            .headers_mut()
            .insert("dav", HeaderValue::from_static("1, 2"));
    }
    response
}

/// `X-OC-Mtime` (Nextcloud, rclone) en secondes Unix, ignoré si invalide.
fn request_mtime(req: &Request) -> Option<OffsetDateTime> {
    let raw = req.headers().get("x-oc-mtime")?.to_str().ok()?;
    let seconds: i64 = raw.trim().parse().ok()?;
    OffsetDateTime::from_unix_timestamp(seconds).ok()
}

fn prevalidate(req: &Request, max_upload: u64) -> Result<(), StatusCode> {
    if req.method() == Method::PUT && content_range_exceeds(req, max_upload) {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    for raw in req.uri().path().split('/') {
        if raw.is_empty() {
            continue;
        }
        let decoded = percent_decode_str(raw).collect::<Vec<u8>>();
        if decoded == b"." || decoded == b".." {
            return Err(StatusCode::FORBIDDEN);
        }
        if decoded.iter().any(|byte| matches!(*byte, b'/' | b'\\' | 0)) {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if is_move(req) && destination_forbidden(req) {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(())
}

/// `Content-Range` d'un PUT partiel dont la taille finale dépasserait `max_upload` → 413 avant toute ouverture.
fn content_range_exceeds(req: &Request, max_upload: u64) -> bool {
    let Some(raw) = req
        .headers()
        .get(header::CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    let Some((unit, spec)) = raw.trim().split_once(' ') else {
        return false;
    };
    if !unit.eq_ignore_ascii_case("bytes") {
        return false;
    }
    let spec = spec.trim();
    let (range, total) = match spec.split_once('/') {
        Some(parts) => parts,
        None => (spec, "*"),
    };
    let Some((start, end)) = range.split_once('-') else {
        // En-tête non reconnu : laissé à dav-server qui répondra 400.
        return false;
    };
    let (Ok(start), Ok(end)) = (start.trim().parse::<u64>(), end.trim().parse::<u64>()) else {
        return false;
    };
    let end_exclusive = end.saturating_add(1);
    let declared = total.trim().parse::<u64>().unwrap_or(end_exclusive);
    declared.max(end_exclusive) > max_upload || start > max_upload
}

/// `Destination` absolu doit viser le même schéma et la même autorité que la requête.
fn destination_forbidden(req: &Request) -> bool {
    let Some(destination) = destination_uri(req) else {
        return true;
    };
    if destination.authority().is_none() {
        return false;
    }
    let authority = req
        .uri()
        .authority()
        .map(|value| value.as_str())
        .or_else(|| {
            req.headers()
                .get(header::HOST)
                .and_then(|value| value.to_str().ok())
        });
    let scheme = req.uri().scheme_str().or_else(|| {
        req.headers()
            .get("x-forwarded-proto")
            .and_then(|value| value.to_str().ok())
    });
    destination.authority().map(|value| value.as_str()) != authority
        || destination.scheme_str() != Some(scheme.unwrap_or("http"))
}
