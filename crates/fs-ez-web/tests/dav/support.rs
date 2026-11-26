use std::time::{Duration, SystemTime};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use fs_ez_app::ViewInput;
use fs_ez_core::datetime::now_local;
use fs_ez_core::{FileId, FilterExpr, TagColor, VirtualFolder, VirtualFolderId, virtual_file_name};

use crate::common::*;

pub(crate) const TAG: &str = "work";
pub(crate) const FILES: &str = "All%20Files";
pub(crate) fn tag_file_uri(tag: &str, alias: &str) -> String {
    format!("/dav/tags/{tag}/{FILES}/{alias}")
}

pub(crate) const VIEW_GROUP: &str = "Divers";
pub(crate) const VIEW: &str = "notes";

pub(crate) struct Dav {
    pub(crate) fixture: Fixture,
    pub(crate) token: String,
    pub(crate) alias: String,
}

pub(crate) async fn setup() -> Dav {
    setup_named("note.txt", b"hello").await
}

pub(crate) async fn setup_named(file_name: &str, content: &[u8]) -> Dav {
    let fixture = Fixture::new(FixtureOptions::default().create_files_dir()).await;
    let path = fixture.root().join(file_name);
    std::fs::write(&path, content).expect("écriture");
    let file = fixture
        .app
        .ensure_file(&path, fs_ez_app::EnsureOptions { force: false })
        .expect("fichier");
    fixture.create_tag(TAG, TagColor::Blue);
    fixture
        .app
        .tag_path(&path, &[TAG.to_string()], false)
        .expect("tag");
    fixture
        .app
        .create_view(ViewInput {
            name: VIEW.to_string(),
            filter: Some(FilterExpr::Tag {
                tag: TAG.to_string(),
                negated: false,
            }),
            icon: None,
            color: None,
            position: None,
            sort: Default::default(),
        })
        .expect("vue");
    let token = fixture
        .app
        .create_api_token("dav", None)
        .expect("jeton")
        .token;
    let alias = virtual_file_name(&file);
    Dav {
        fixture,
        token,
        alias,
    }
}

pub(crate) async fn inbox_fixture() -> (Fixture, String) {
    let fixture = Fixture::new(
        FixtureOptions::default()
            .create_files_dir()
            .create_inbox_dir(),
    )
    .await;
    let token = fixture
        .app
        .create_api_token("dav", None)
        .expect("jeton")
        .token;
    (fixture, token)
}

pub(crate) fn child(app: &App, parent: Option<VirtualFolderId>, name: &str) -> VirtualFolder {
    app.virtual_folder_children(parent)
        .expect("dossiers virtuels")
        .into_iter()
        .find(|folder| folder.name == name)
        .unwrap_or_else(|| panic!("dossier {name} absent"))
}

pub(crate) fn inbox_date(app: &App) -> VirtualFolder {
    let inbox = child(app, None, "inbox");
    child(app, Some(inbox.id), &now_local().date().to_string())
}

pub(crate) fn files_of(app: &App, folder: VirtualFolderId) -> Vec<FileId> {
    app.files_in_virtual_folder(Some(folder), 0, 50)
        .expect("fichiers du dossier")
        .items
        .into_iter()
        .map(|item| item.file.id)
        .collect()
}

pub(crate) fn folder_names(app: &App, parent: Option<VirtualFolderId>) -> Vec<String> {
    app.virtual_folder_children(parent)
        .expect("dossiers virtuels")
        .into_iter()
        .map(|folder| folder.name)
        .collect()
}

fn auth(token: &str) -> header::HeaderValue {
    header::HeaderValue::from_str(&format!(
        "Basic {}",
        STANDARD.encode(format!("user:{token}"))
    ))
    .expect("auth")
}

pub(crate) fn dav_request(method: Method, uri: &str, token: &str, body: Body) -> Request<Body> {
    let mut request = request(method, uri, body);
    request
        .headers_mut()
        .insert(header::AUTHORIZATION, auth(token));
    request
}

pub(crate) fn bearer_request(method: Method, uri: &str, token: &str) -> Request<Body> {
    let mut request = request(method, uri, Body::empty());
    request.headers_mut().insert(
        header::AUTHORIZATION,
        header::HeaderValue::from_str(&format!("Bearer {token}")).expect("bearer"),
    );
    request
}

pub(crate) fn move_request(
    uri: &str,
    destination: &str,
    token: &str,
    host: Option<&str>,
) -> Request<Body> {
    let mut request = dav_request(
        Method::from_bytes(b"MOVE").expect("méthode"),
        uri,
        token,
        Body::empty(),
    );
    request.headers_mut().insert(
        "destination",
        header::HeaderValue::from_str(destination).expect("destination"),
    );
    if let Some(host) = host {
        request.headers_mut().insert(
            header::HOST,
            header::HeaderValue::from_str(host).expect("host"),
        );
    }
    request
}

pub(crate) fn propfind(uri: &str, token: &str, depth: &str) -> Request<Body> {
    let mut request = dav_request(
        Method::from_bytes(b"PROPFIND").expect("méthode"),
        uri,
        token,
        Body::empty(),
    );
    request.headers_mut().insert(
        "depth",
        header::HeaderValue::from_str(depth).expect("depth"),
    );
    request
}

pub(crate) async fn body_bytes(response: Response<Body>) -> Vec<u8> {
    axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("corps")
        .to_vec()
}

pub(crate) async fn assert_multi_status(response: Response<Body>) -> String {
    assert_eq!(response.status(), StatusCode::MULTI_STATUS);
    body_text(response).await
}

pub(crate) fn assert_mtime(path: &std::path::Path, seconds: u64) {
    assert_eq!(
        std::fs::metadata(path)
            .expect("meta")
            .modified()
            .expect("mtime"),
        SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
    );
}

pub(crate) fn assert_href(body: &str, path: &str) {
    assert!(body.contains(&format!("<D:href>{path}</D:href>")), "{body}");
}

pub(crate) fn range_request(method: Method, uri: &str, token: &str, range: &str) -> Request<Body> {
    let mut request = dav_request(method, uri, token, Body::empty());
    request.headers_mut().insert(
        header::RANGE,
        header::HeaderValue::from_str(range).expect("range"),
    );
    request
}

pub(crate) fn propfind_with_body(
    uri: &str,
    token: &str,
    depth: &str,
    body: &'static str,
) -> Request<Body> {
    let mut request = dav_request(
        Method::from_bytes(b"PROPFIND").expect("méthode"),
        uri,
        token,
        Body::from(body),
    );
    request.headers_mut().insert(
        "depth",
        header::HeaderValue::from_str(depth).expect("depth"),
    );
    request.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/xml"),
    );
    request
}

pub(crate) fn with_header(
    mut request: Request<Body>,
    name: &'static str,
    value: &str,
) -> Request<Body> {
    request.headers_mut().insert(
        header::HeaderName::from_static(name),
        header::HeaderValue::from_str(value).expect("valeur"),
    );
    request
}

pub(crate) fn content_range_put(
    uri: &str,
    token: &str,
    body: &'static str,
    length: &str,
    range: &str,
) -> Request<Body> {
    let request = dav_request(Method::PUT, uri, token, Body::from(body));
    let request = with_header(request, "content-length", length);
    with_header(request, "content-range", range)
}

pub(crate) fn mkcol() -> Method {
    Method::from_bytes(b"MKCOL").expect("méthode")
}

pub(crate) async fn assert_put_created(
    router: &Router,
    uri: &str,
    token: &str,
    body: &'static str,
) {
    let response = send(
        router,
        dav_request(Method::PUT, uri, token, Body::from(body)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED, "{uri}");
}

pub(crate) async fn assert_mkcol_created(router: &Router, directory: &str, token: &str) {
    let response = send(
        router,
        dav_request(mkcol(), directory, token, Body::empty()),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED, "{directory}");
}

pub(crate) async fn dav_get(router: &Router, uri: &str, token: &str) -> Response<Body> {
    send(router, dav_request(Method::GET, uri, token, Body::empty())).await
}

pub(crate) async fn move_folder(
    router: &Router,
    from: &str,
    to: &str,
    token: &str,
) -> Response<Body> {
    send(router, move_request(from, to, token, None)).await
}

pub(crate) async fn propfind_body(router: &Router, path: &str, token: &str, depth: &str) -> String {
    body_text(send(router, propfind(path, token, depth)).await).await
}

pub(crate) fn assert_no_child(
    app: &App,
    parent: Option<VirtualFolderId>,
    absent: &[&str],
    message: &str,
) {
    let names = folder_names(app, parent);
    assert!(
        !names.iter().any(|name| absent.contains(&name.as_str())),
        "{message}"
    );
}
