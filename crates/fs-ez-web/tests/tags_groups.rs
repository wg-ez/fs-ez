//! Fiche 28 : tags, groupes, couleurs, sidebar OOB et drag & drop simulé.

#[path = "tags_groups/groups.rs"]
mod groups;
#[path = "tags_groups/security.rs"]
mod security;
#[path = "tags_groups/sidebar.rs"]
mod sidebar;
#[path = "tags_groups/sidebar_groups.rs"]
mod sidebar_groups;
#[path = "tags_groups/sidebar_tags.rs"]
mod sidebar_tags;
#[path = "tags_groups/tags.rs"]
mod tags;

mod common;

use common::*;
use fs_ez_core::{FileId, FileKind, Fingerprint, NewFile, TagColor};
use time::OffsetDateTime;

async fn fixture() -> Fixture {
    common::default_fixture().await
}

trait GroupFiles {
    fn add_file(&self, name: &str, tags: &[&str]) -> FileId;
}

impl GroupFiles for Fixture {
    fn add_file(&self, name: &str, tags: &[&str]) -> FileId {
        let path = self.root().join(name);
        std::fs::write(&path, b"x").expect("fichier");
        let file = self
            .app
            .store
            .insert_file(&NewFile {
                path: path.clone(),
                path_display: path.to_string_lossy().into_owned(),
                name: name.to_string(),
                dir: self.root().to_string_lossy().into_owned(),
                size: 1,
                mtime: OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("date"),
                kind: FileKind::File,
                fingerprint: Some(Fingerprint::new(format!("fp-{name}")).expect("empreinte")),
            })
            .expect("fichier");
        if !tags.is_empty() {
            let tags: Vec<String> = tags.iter().map(|tag| (*tag).to_string()).collect();
            self.app.apply_tags(file.id, &tags, true).expect("tags");
        }
        file.id
    }
}

async fn post_hx_target(
    fixture: &Fixture,
    uri: &str,
    body: &str,
    target: &'static str,
) -> Response<Body> {
    let mut request = with_hx(with_cookie(post_form(uri, body), &fixture.cookie));
    request
        .headers_mut()
        .insert("hx-target", header::HeaderValue::from_static(target));
    fixture.send(request).await
}

fn assert_fragment_response(response: &Response<Body>) {
    let trigger = response
        .headers()
        .get("hx-trigger")
        .and_then(|value| value.to_str().ok())
        .expect("hx-trigger");
    let events: Vec<&str> = trigger.split(',').map(str::trim).collect();
    assert!(events.contains(&"tags-changed"), "{trigger}");
    assert!(events.contains(&"data-changed"), "{trigger}");
}

async fn post_sidebar(fixture: &Fixture, uri: &str, body: &str) -> Response<Body> {
    let response = post_hx_target(fixture, uri, body, "sidebar").await;
    assert_eq!(response.status(), StatusCode::OK);
    response
}

fn assert_no_sql_leak(body: &str) {
    assert!(!body.contains("UNIQUE"), "{body}");
    assert!(!body.contains("constraint"), "{body}");
    assert!(!body.contains("SQL"), "{body}");
}

fn assert_sidebar_oob(body: &str) {
    assert!(body.contains("hx-swap-oob=\"outerHTML\""), "{body}");
}
