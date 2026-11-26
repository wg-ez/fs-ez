//! Tests d'integration de la liste à scroll virtuel : tri, recherche, filtres, vues et chunks.

#[path = "files_list/filters.rs"]
mod filters;
#[path = "files_list/htmx.rs"]
mod htmx;
#[path = "files_list/listing.rs"]
mod listing;
#[path = "files_list/pagination.rs"]
mod pagination;
#[path = "files_list/security.rs"]
mod security;
#[path = "files_list/views.rs"]
mod views;

mod common;

use std::collections::HashSet;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;

use common::*;
use fs_ez_core::{FileKind, FilterExpr, NewFile, Sort, SortDir, SortField};
use time::{Duration, OffsetDateTime};

async fn fixture(page_size: Option<usize>) -> Fixture {
    let options = match page_size {
        Some(size) => FixtureOptions::default().page_size(size),
        None => FixtureOptions::default(),
    };
    let mut fixture = Fixture::new(options).await;

    let base = OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("timestamp");
    let sizes = [512_u64, 4096, 1_500_000];
    for index in 0..25_u64 {
        let name = if index == 0 {
            "Rapport-annuel.txt".to_string()
        } else {
            format!("facture-{index:02}.txt")
        };
        let path = fixture.root().join(&name);
        let file = fixture
            .app
            .store
            .insert_file(&NewFile {
                path: path.clone(),
                path_display: path.to_string_lossy().into_owned(),
                name,
                dir: fixture.root().to_string_lossy().into_owned(),
                size: sizes[(index % 3) as usize],
                mtime: base + Duration::days((index % 3) as i64),
                kind: FileKind::File,
                fingerprint: None,
            })
            .expect("fichier");
        let tag = if index <= 9 {
            Some("facture")
        } else if index <= 14 {
            Some("archive")
        } else {
            None
        };
        if let Some(name) = tag {
            fixture
                .app
                .apply_tags(file.id, &[name.to_string()], true)
                .expect("tag");
        }
    }

    fixture.view = Some(
        fixture
            .app
            .create_view(fs_ez_app::ViewInput {
                name: "Factures".to_string(),
                filter: Some(FilterExpr::Tag {
                    tag: "facture".to_string(),
                    negated: false,
                }),
                icon: None,
                color: None,
                position: None,
                sort: Sort {
                    field: SortField::Name,
                    dir: SortDir::Desc,
                },
            })
            .expect("vue")
            .id,
    );
    fixture
}
