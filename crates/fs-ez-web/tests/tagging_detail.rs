//! Tests d'integration du retrait de tag et de l'edition en masse.

mod common;

use common::*;
use fs_ez_core::{FileId, FileKind, Fingerprint, NewFile};
use time::OffsetDateTime;

async fn fixture() -> Fixture {
    common::default_fixture().await
}

trait TestFiles {
    fn add_file(&self, name: &str, content: &[u8], tags: &[&str]) -> FileId;
}

impl TestFiles for Fixture {
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
                mtime: OffsetDateTime::UNIX_EPOCH,
                kind: FileKind::File,
                fingerprint: Some(Fingerprint::new(format!("fp-{name}")).expect("empreinte")),
            })
            .expect("fichier");
        if !tags.is_empty() {
            let tags: Vec<String> = tags.iter().map(|tag| (*tag).to_string()).collect();
            self.app.apply_tags(file.id, &tags, true).expect("tag");
        }
        file.id
    }
}

#[path = "tagging_detail/bulk.rs"]
mod bulk;
#[path = "tagging_detail/security.rs"]
mod security;
#[path = "tagging_detail/tagging.rs"]
mod tagging;
