//! Tests de parite entre le compilateur SQL et l'evaluateur memoire de la fiche 07.

use std::collections::BTreeSet;
use std::path::PathBuf;

use fs_ez_core::{FileId, FilterExpr, Sort, SortDir, SortField, TagId};
use fs_ez_query::{EvalTag, evaluate, parse};
use fs_ez_sqlite::{SqliteStore, db, query_sql};
use fs_ez_store::{FileListQuery, FileStore};
use rusqlite::params_from_iter;
use time::macros::datetime;
use time::{Duration, OffsetDateTime};

mod common;

use common::{list_query, new_file, store};

fn insert_file(store: &SqliteStore, index: usize, mtime: OffsetDateTime) -> FileId {
    // Noms dupliques par paires pour exercer le tie-break `id` de la pagination.
    let path = PathBuf::from(format!("/data/d{index}/f{:02}.txt", index / 2));
    store
        .insert_file(&new_file(&path, None, (index as u64) * 10, mtime))
        .expect("insertion")
        .id
}

fn apply(store: &SqliteStore, file: FileId, name: &str, at: OffsetDateTime) {
    let tag: TagId = common::tag(store, name);
    store.apply_tag(file, tag, true, at).expect("application");
}

struct Fixture {
    store: SqliteStore,
    ids: Vec<FileId>,
}

fn fixture() -> Fixture {
    let store = store();
    let base = datetime!(2026-01-01 00:00 UTC);
    let ids: Vec<FileId> = (0..20)
        .map(|index| insert_file(&store, index, base + Duration::minutes(index as i64 * 7)))
        .collect();
    for (index, file) in ids.iter().enumerate() {
        let at = base + Duration::minutes(index as i64);
        apply(&store, *file, "projet", at);
        if index % 2 == 0 {
            apply(&store, *file, "facture", at);
        }
        if index % 3 == 0 {
            apply(&store, *file, "paye", at);
        }
        if index % 7 == 0 {
            apply(&store, *file, "urgent", at);
        }
        if index % 10 == 0 {
            apply(&store, *file, "archive", at);
        }
    }
    Fixture { store, ids }
}

fn filter_query(filter: Option<FilterExpr>) -> FileListQuery {
    list_query(
        filter,
        Sort {
            field: SortField::Id,
            dir: SortDir::Asc,
        },
        0,
        500,
    )
}

fn sql_ids(store: &SqliteStore, expr: &FilterExpr) -> BTreeSet<i64> {
    let page = store
        .list_files(&filter_query(Some(expr.clone())))
        .expect("liste");
    page.items.iter().map(|item| item.file.id.0).collect()
}

fn memory_ids(store: &SqliteStore, expr: &FilterExpr) -> BTreeSet<i64> {
    let mut ids = BTreeSet::new();
    for file in store.all_files().expect("fichiers") {
        let tags = store.tags_for_file(file.id).expect("tags");
        let eval_tags: Vec<EvalTag> = tags
            .iter()
            .map(|applied| EvalTag::new(&applied.tag.name).expect("nom valide"))
            .collect();
        if evaluate(expr, &eval_tags) {
            ids.insert(file.id.0);
        }
    }
    ids
}

#[test]
fn sql_matches_memory_evaluator() {
    let fixture = fixture();
    let queries = [
        "projet",
        "facture and paye",
        "not facture",
        "not (facture or paye)",
        "projet and not archive",
        "urgent or archive",
    ];
    let mut non_empty = 0;
    for raw in queries {
        let expr = parse(raw).expect(raw);
        let expected = memory_ids(&fixture.store, &expr);
        let actual = sql_ids(&fixture.store, &expr);
        assert_eq!(actual, expected, "{raw}");
        if !expected.is_empty() {
            non_empty += 1;
        }
    }
    assert_eq!(
        non_empty,
        queries.len(),
        "un filtre de test ne selectionne rien"
    );
    assert_eq!(
        sql_ids(&fixture.store, &parse("projet").expect("requete")).len(),
        20
    );
}

#[test]
fn only_tag_matches_exact_tag_sets() {
    let store = store();
    let at = datetime!(2026-01-01 00:00 UTC);
    let alpha = insert_file(&store, 0, at);
    let alpha_beta = insert_file(&store, 1, at);
    let beta = insert_file(&store, 2, at);
    apply(&store, alpha, "alpha", at);
    apply(&store, alpha_beta, "alpha", at);
    apply(&store, alpha_beta, "beta", at);
    apply(&store, beta, "beta", at);

    let expr = parse("only alpha").expect("requete");
    assert_eq!(sql_ids(&store, &expr), BTreeSet::from([alpha.0]));
    assert_eq!(sql_ids(&store, &expr), memory_ids(&store, &expr));

    let negated = parse("not only alpha").expect("requete");
    assert_eq!(
        sql_ids(&store, &negated),
        BTreeSet::from([alpha_beta.0, beta.0])
    );
    assert_eq!(sql_ids(&store, &negated), memory_ids(&store, &negated));

    let exact = parse("exactly (alpha and beta)").expect("requete");
    assert_eq!(sql_ids(&store, &exact), BTreeSet::from([alpha_beta.0]));
    assert_eq!(sql_ids(&store, &exact), memory_ids(&store, &exact));

    let exact_negated = parse("not exactly (alpha and beta)").expect("requete");
    assert_eq!(
        sql_ids(&store, &exact_negated),
        BTreeSet::from([alpha.0, beta.0])
    );
    assert_eq!(
        sql_ids(&store, &exact_negated),
        memory_ids(&store, &exact_negated)
    );
}

#[test]
fn offset_pagination_covers_every_sort() {
    let fixture = fixture();
    for field in [
        SortField::Id,
        SortField::Name,
        SortField::Size,
        SortField::Mtime,
    ] {
        for dir in [SortDir::Asc, SortDir::Desc] {
            let sort = Sort { field, dir };
            let full: Vec<i64> = fixture
                .store
                .list_files(&FileListQuery {
                    sort,
                    limit: 500,
                    ..filter_query(None)
                })
                .expect("liste complete")
                .items
                .iter()
                .map(|item| item.file.id.0)
                .collect();
            let mut paged = Vec::new();
            let mut offset = 0;
            loop {
                let page = fixture
                    .store
                    .list_files(&FileListQuery {
                        sort,
                        offset,
                        limit: 3,
                        ..filter_query(None)
                    })
                    .expect("page");
                paged.extend(page.items.iter().map(|item| item.file.id.0));
                match page.next_offset {
                    Some(next) => offset = next,
                    None => break,
                }
            }
            assert_eq!(paged, full, "{field:?} {dir:?}");
            assert_eq!(paged.len(), fixture.ids.len(), "{field:?} {dir:?}");
            assert_eq!(
                paged.iter().copied().collect::<BTreeSet<_>>().len(),
                paged.len(),
                "doublon de pagination {field:?} {dir:?}"
            );
        }
    }
}

#[test]
fn tag_predicate_uses_indexes() {
    let conn = db::open_in_memory().expect("base");
    conn.execute(
        "INSERT INTO file (path, path_display, name, dir, size, mtime, kind, created_at, updated_at)
         VALUES ('/a', '/a', 'a', '/', 0, '2026-01-01T00:00:00Z', 'file',
                 '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    )
    .expect("fichier");
    conn.execute(
        "INSERT INTO tag (name, name_norm, created_at) VALUES ('facture', 'facture', '2026-01-01T00:00:00Z')",
        [],
    )
    .expect("tag");
    conn.execute(
        "INSERT INTO file_tag (file_id, tag_id, created_at) VALUES (1, 1, '2026-01-01T00:00:00Z')",
        [],
    )
    .expect("application");

    let expr = FilterExpr::Tag {
        tag: "facture".to_string(),
        negated: false,
    };
    let (sql, values) = query_sql::filter_subquery(&expr).expect("compilation");
    let mut statement = conn
        .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
        .expect("preparation");
    let plan: Vec<String> = statement
        .query_map(params_from_iter(values.iter()), |row| row.get(3))
        .expect("plan")
        .collect::<rusqlite::Result<_>>()
        .expect("lecture");
    let plan = plan.join("\n");
    assert!(!plan.contains("SCAN ft"), "{plan}");
    assert!(!plan.contains("SCAN file_tag"), "{plan}");
    assert!(plan.contains("SEARCH"), "{plan}");
}
