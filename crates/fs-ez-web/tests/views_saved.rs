//! Fiche 29 : vues sauvegardées, constructeur visuel et aperçu.

#[path = "views_saved/create.rs"]
mod create;
#[path = "views_saved/duplicate.rs"]
mod duplicate;
#[path = "views_saved/edit.rs"]
mod edit;
#[path = "views_saved/forms.rs"]
mod forms;
#[path = "views_saved/security.rs"]
mod security;
#[path = "views_saved/sidebar.rs"]
mod sidebar;
#[path = "views_saved/validation.rs"]
mod validation;

mod common;

use common::*;
use fs_ez_app::ViewInput;
use fs_ez_core::{FileId, FileKind, FilterExpr, Fingerprint, NewFile, Sort, SortDir, SortField};
use time::OffsetDateTime;

async fn fixture() -> Fixture {
    common::default_fixture().await
}

trait ViewFiles {
    fn add_file(&self, name: &str, tags: &[&str]) -> FileId;
}

impl ViewFiles for Fixture {
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

fn facture_not_paye_json() -> String {
    serde_json::to_string(&FilterExpr::And {
        items: vec![
            FilterExpr::Tag {
                tag: "facture".to_string(),
                negated: false,
            },
            FilterExpr::Tag {
                tag: "paye".to_string(),
                negated: true,
            },
        ],
    })
    .expect("json")
}

async fn create_view(
    fixture: &Fixture,
    name: &str,
    filter_json: &str,
    query: &str,
) -> Response<Body> {
    let body = form(&[
        ("name", name),
        ("filter_json", filter_json),
        ("query", query),
        ("sort", "id:asc"),
    ]);
    fixture.post_hx("/views", &body).await
}
