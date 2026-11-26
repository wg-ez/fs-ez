//! Porte de phase 2 : auth, CSRF, liste, tagging, securite HTTP, snapshots.

#[path = "router_phase2/auth.rs"]
mod auth;
#[path = "router_phase2/files.rs"]
mod files;
#[path = "router_phase2/pagination.rs"]
mod pagination;
#[path = "router_phase2/security.rs"]
mod security;
#[path = "router_phase2/tags.rs"]
mod tags;

mod common;

use common::*;
use fs_ez_core::{FileId, FileKind, Fingerprint, NewFile, TagColor};
use fs_ez_web::views::files::format_mtime;
use time::OffsetDateTime;

const FIXED_MTIME: i64 = 1_700_000_000;

async fn web_fixture() -> Fixture {
    Fixture::new(FixtureOptions::default()).await
}

async fn paged_fixture(page_size: usize) -> Fixture {
    Fixture::new(FixtureOptions::default().page_size(page_size)).await
}

trait AddFile {
    fn add_file(&self, name: &str, content: &[u8], tags: &[&str]) -> FileId;
}

impl AddFile for Fixture {
    fn add_file(&self, name: &str, content: &[u8], tags: &[&str]) -> FileId {
        let path = self.root().join(name);
        std::fs::write(&path, content).expect("fichier");
        let file = self
            .app
            .store
            .insert_file(&NewFile {
                path: path.clone(),
                path_display: path.to_string_lossy().into_owned(),
                name: name.to_string(),
                dir: self.root().to_string_lossy().into_owned(),
                size: content.len() as u64,
                mtime: fixed_mtime(),
                kind: FileKind::File,
                fingerprint: Some(Fingerprint::new(format!("fp-{name}")).expect("empreinte")),
            })
            .expect("fichier");
        if !tags.is_empty() {
            self.app
                .apply_tags(file.id, &tag_inputs(tags), true)
                .expect("tags");
        }
        file.id
    }
}

fn fixed_mtime() -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(FIXED_MTIME).expect("timestamp")
}

fn tag_inputs(tags: &[&str]) -> Vec<String> {
    tags.iter().map(|tag| (*tag).to_string()).collect()
}

fn between(html: &str, start: &str, end: &str) -> String {
    let Some(open) = html.find(start) else {
        return String::new();
    };
    let rest = &html[open..];
    match rest.find(end) {
        Some(close) => rest[..close + end.len()].to_string(),
        None => rest.to_string(),
    }
}

fn redact_common(html: &str, root: &str) -> String {
    html.replace(root, "[root]")
        .replace(&format_mtime(fixed_mtime()), "[date]")
}

fn redact_ids(html: &str, ids: &[FileId]) -> String {
    let mut out = html.to_string();
    for id in ids {
        let id = id.to_string();
        out = out.replace(&format!("value=\"{id}\""), "value=\"[id]\"");
        out = out.replace(&format!("data-file-id=\"{id}\""), "data-file-id=\"[id]\"");
        out = out.replace(
            &format!("/partials/files/{id}/rename"),
            "/partials/files/[id]/rename",
        );
        out = out.replace(&format!("id=\"file-tags-{id}\""), "id=\"file-tags-[id]\"");
    }
    out
}

fn assert_no_secrets(snapshot: &str, token: &str) {
    assert!(
        !snapshot.contains(PASSWORD),
        "mot de passe dans le snapshot"
    );
    assert!(!snapshot.contains(token), "jeton dans le snapshot");
}

#[tokio::test]
async fn snapshots_cover_shell_pagination_and_login() {
    let fixture = web_fixture().await;
    let first = fixture.add_file("Rapport-annuel.txt", b"hello", &["facture"]);
    let second = fixture.add_file("facture-02.txt", b"xx", &["archive"]);
    let third = fixture.add_file("notes.txt", b"x", &[]);
    for tag in ["facture", "archive"] {
        fixture
            .app
            .set_tag_color(tag, TagColor::Neutral, None)
            .expect("couleur");
    }
    let ids = [first, second, third];
    let root = fixture.dir.path().to_string_lossy().into_owned();

    let (status, html) = fixture.page("/").await;
    assert_eq!(status, StatusCode::OK);
    let main = between(&html, "<main ", "</main>");
    assert!(!main.is_empty());
    let main = redact_ids(&redact_common(&main, &root), &ids);
    assert_no_secrets(&main, &fixture.cookie);
    insta::assert_snapshot!("phase2_main_block", main);

    let paged = paged_fixture(2).await;
    let mut paged_ids = Vec::new();
    for index in 1..=5 {
        paged_ids.push(paged.add_file(&format!("page-{index}.txt"), b"x", &[]));
    }
    let (status, rows) = paged
        .page("/partials/files/rows?sort=id%3Aasc&offset=0&limit=2")
        .await;
    assert_eq!(status, StatusCode::OK);
    let paged_root = paged.dir.path().to_string_lossy().into_owned();
    let rows = redact_ids(&redact_common(&rows, &paged_root), &paged_ids);
    insta::assert_snapshot!("phase2_pagination", rows);

    let response = send(&fixture.router(), get("/login")).await;
    assert_eq!(response.status(), StatusCode::OK);
    let login = body_text(response).await;
    insta::assert_snapshot!("phase2_login_page", between(&login, "<main ", "</main>"));

    let response = send(
        &fixture.router(),
        post_form("/login", &login_body("mauvais-mot-de-passe")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let error = body_text(response).await;
    insta::assert_snapshot!("phase2_login_error", between(&error, "<main ", "</main>"));

    let response = send(
        &fixture.router(),
        post_form("/login", &login_body(PASSWORD)),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let raw_cookie = header_string(&response, header::SET_COOKIE);
    let redacted = redact_cookie(&raw_cookie);
    assert_no_secrets(&redacted, &session_token(&raw_cookie));
    let summary = format!(
        "status: {}\nlocation: {}\nset-cookie: {}",
        response.status().as_u16(),
        location(&response),
        redacted
    );
    insta::assert_snapshot!("phase2_login_success", summary);
}

fn redact_cookie(raw: &str) -> String {
    let mut parts = raw.split(';');
    let first = parts.next().unwrap_or_default();
    let name = first.split('=').next().unwrap_or_default();
    let rest: String = parts.map(|part| format!(";{part}")).collect();
    format!("{name}=[token]{rest}")
}
