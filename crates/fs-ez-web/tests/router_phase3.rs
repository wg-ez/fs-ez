//! Porte de phase 3 : sidebar, groupes, tags, vues, reglages, themes et i18n.

mod common;

use common::*;
use fs_ez_app::ViewInput;
use fs_ez_core::{
    FileId, FileKind, FilterExpr, Fingerprint, NewFile, Sort, SortDir, SortField, TagColor, ViewId,
};
use time::OffsetDateTime;

const FILE_BODY: &str = "SECRET-BODY";
const OLD_STAMP: &str = "2000-01-01T00:00:00Z";

async fn fixture() -> Fixture {
    Fixture::new(FixtureOptions::default().persistent_db()).await
}

async fn seeded() -> Fixture {
    let fixture = fixture().await;
    fixture.seed();
    fixture
}

trait Seed {
    fn seed(&self);
    #[allow(clippy::too_many_arguments)]
    fn create_view(
        &self,
        name: &str,
        filter: FilterExpr,
        query: &str,
        position: Option<i64>,
        sort: Sort,
        color: Option<TagColor>,
    ) -> ViewId;
    fn add_file(&self, name: &str, tags: &[&str]) -> FileId;
    fn tag_file(&self, file: FileId, tags: &[&str]);
}

impl Seed for Fixture {
    /// 12 tags, 3 groupes, 4 vues, 10 fichiers tagges, 2 libres et 2 recents.
    fn seed(&self) {
        for color in TagColor::ALL {
            self.create_tag(&format!("c-{}", color.as_token()), color);
        }

        self.add_file("facture-due.txt", &["facture", "c-blue", "projet-alpha"]);
        self.add_file(
            "facture-payee.txt",
            &["facture", "paye", "c-green", "projet-beta"],
        );
        self.add_file("archive-2025.txt", &["archive", "c-red"]);
        self.add_file("photo-indigo.txt", &["c-indigo"]);
        self.add_file("projet-alpha.txt", &["c-violet", "projet-alpha"]);
        self.add_file("note-pink.txt", &["c-pink"]);
        self.add_file("manuel-brown.txt", &["c-brown"]);
        self.add_file("config-cyan.txt", &["c-cyan"]);
        let teal = self.add_file("scan-teal.txt", &[]);
        let amber = self.add_file("rapport-amber.txt", &[]);

        let connection = db::open(&self.db_path).expect("base secondaire");
        connection
            .execute("UPDATE file_tag SET created_at = ?1", [OLD_STAMP])
            .expect("anciennete");
        drop(connection);
        self.tag_file(teal, &["c-teal"]);
        self.tag_file(amber, &["c-amber"]);

        let finance = self
            .app
            .create_group("Finance", Some(TagColor::Amber))
            .expect("groupe");
        for tag in ["c-blue", "c-green", "c-amber"] {
            self.app.assign_tag(finance.id, tag).expect("assignation");
        }
        let projets = self
            .app
            .create_group("Projets", Some(TagColor::Indigo))
            .expect("groupe");
        for tag in ["c-indigo", "c-violet"] {
            self.app.assign_tag(projets.id, tag).expect("assignation");
        }
        let divers = self.app.create_group("Divers", None).expect("groupe");
        for tag in ["c-red", "c-teal"] {
            self.app.assign_tag(divers.id, tag).expect("assignation");
        }

        self.create_view(
            "Factures",
            FilterExpr::And {
                items: vec![tag_filter("facture", false), tag_filter("paye", true)],
            },
            "facture and not paye",
            Some(2),
            Sort {
                field: SortField::Name,
                dir: SortDir::Asc,
            },
            Some(TagColor::Amber),
        );
        self.create_view(
            "Projets",
            tag_filter("projet-alpha", false),
            "projet-alpha",
            Some(1),
            Sort {
                field: SortField::Id,
                dir: SortDir::Desc,
            },
            None,
        );
        self.create_view(
            "Archives",
            tag_filter("archive", false),
            "archive",
            None,
            Sort {
                field: SortField::Mtime,
                dir: SortDir::Desc,
            },
            None,
        );
        self.create_view(
            "Tout",
            FilterExpr::Or {
                items: vec![tag_filter("facture", false), tag_filter("archive", false)],
            },
            "facture or archive",
            None,
            Sort {
                field: SortField::Size,
                dir: SortDir::Asc,
            },
            None,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn create_view(
        &self,
        name: &str,
        filter: FilterExpr,
        _query: &str,
        position: Option<i64>,
        sort: Sort,
        color: Option<TagColor>,
    ) -> ViewId {
        self.app
            .create_view(ViewInput {
                name: name.to_string(),
                filter: Some(filter),
                icon: None,
                color,
                position,
                sort,
            })
            .expect("vue")
            .id
    }

    fn add_file(&self, name: &str, tags: &[&str]) -> FileId {
        let path = self.root().join(name);
        std::fs::write(&path, FILE_BODY.as_bytes()).expect("fichier");
        let file = self
            .app
            .store
            .insert_file(&NewFile {
                path: path.clone(),
                path_display: path.to_string_lossy().into_owned(),
                name: name.to_string(),
                dir: self.root().to_string_lossy().into_owned(),
                size: FILE_BODY.len() as u64,
                mtime: OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("date"),
                kind: FileKind::File,
                fingerprint: Some(Fingerprint::new(format!("fp-{name}")).expect("empreinte")),
            })
            .expect("fichier");
        self.tag_file(file.id, tags);
        file.id
    }

    fn tag_file(&self, file: FileId, tags: &[&str]) {
        if tags.is_empty() {
            return;
        }
        for tag in tags {
            self.create_tag(tag, TagColor::Neutral);
        }
        let tags: Vec<String> = tags.iter().map(|tag| (*tag).to_string()).collect();
        self.app.apply_tags(file, &tags, true).expect("tags");
    }
}

fn tag_filter(tag: &str, negated: bool) -> FilterExpr {
    FilterExpr::Tag {
        tag: tag.to_string(),
        negated,
    }
}

fn redact_random_create_colors(input: &str) -> String {
    const TOKENS: [&str; 12] = [
        "neutral", "red", "orange", "amber", "green", "teal", "cyan", "blue", "indigo", "violet",
        "pink", "brown",
    ];
    fn redact_block(block: &str, tokens: &[&str], drop_selection: bool) -> String {
        let mut block = block.to_string();
        for token in tokens {
            block = block.replace(&format!("dot--{token}"), "dot--[color]");
            block = block.replace(&format!("value=\"{token}\""), "value=\"[color]\"");
        }
        if drop_selection {
            block = block.replace(" is-on", "");
            block = block.replace(" checked", "");
        }
        block
    }
    let mut output = input.to_string();
    for (marker, close, drop_selection) in [
        (
            "<li class=\"sidebar__row sidebar__row--create\">",
            "</li>",
            false,
        ),
        ("<form class=\"sidebar__group-create\"", "</form>", false),
        (
            "<div class=\"sidebar__view-editor__colors\"",
            "</div>",
            true,
        ),
    ] {
        let mut rest = output.as_str();
        let mut next = String::with_capacity(rest.len());
        while let Some(index) = rest.find(marker) {
            next.push_str(&rest[..index]);
            let block = &rest[index..];
            let end = block
                .find(close)
                .map_or(block.len(), |end| end + close.len());
            next.push_str(&redact_block(&block[..end], &TOKENS, drop_selection));
            rest = &block[end..];
        }
        next.push_str(rest);
        output = next;
    }
    output
}

fn redact_numbered(input: &str) -> String {
    let prefixes = [
        "/views/",
        "/partials/tags/",
        "/partials/groups/",
        "data-group=\"",
        "data-group-list=\"",
        "data-group-row=\"",
        "data-group-tag-open=\"",
        "name=\"group\" value=\"",
        "sidebar-group-name-",
        "sidebar-tag-create-",
        "sidebar-tag-name-",
    ];
    let mut output = input.to_string();
    for prefix in prefixes {
        output = redact_prefix(&output, prefix);
    }
    output
}

fn redact_prefix(input: &str, prefix: &str) -> String {
    let mut output = String::new();
    let mut rest = input;
    while let Some(index) = rest.find(prefix) {
        let after = &rest[index + prefix.len()..];
        let digits = after.chars().take_while(char::is_ascii_digit).count();
        output.push_str(&rest[..index + prefix.len()]);
        if digits > 0 {
            output.push_str("[id]");
            rest = &after[digits..];
        } else {
            rest = after;
        }
    }
    output.push_str(rest);
    output
}

#[path = "router_phase3/classic_forms.rs"]
mod classic_forms;
#[path = "router_phase3/groups.rs"]
mod groups;
#[path = "router_phase3/locales.rs"]
mod locales;
#[path = "router_phase3/preferences.rs"]
mod preferences;
#[path = "router_phase3/security.rs"]
mod security;
#[path = "router_phase3/sidebar.rs"]
mod sidebar;
#[path = "router_phase3/tags.rs"]
mod tags;
#[path = "router_phase3/views.rs"]
mod views;

#[tokio::test]
async fn snapshots_cover_sidebar() {
    let fixture = seeded().await;

    let (status, sidebar) = fixture.page("/partials/sidebar").await;
    assert_eq!(status, StatusCode::OK);
    let sidebar = redact_random_create_colors(&redact_numbered(&sidebar));
    insta::assert_snapshot!("phase3_sidebar", sidebar);

    let (status, _) = fixture
        .page(&format!("/views/{}/edit", fixture.view_id("Factures")))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let response = fixture.post_hx("/partials/sidebar/toggle", "").await;
    assert_eq!(response.status(), StatusCode::OK);
    let reduced = redact_random_create_colors(&redact_numbered(&body_text(response).await));
    insta::assert_snapshot!("phase3_sidebar_reduced", reduced);
}
