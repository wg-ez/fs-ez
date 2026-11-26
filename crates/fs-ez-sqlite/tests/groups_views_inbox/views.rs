use super::*;

#[test]
fn views_create_get_list_update_and_delete() {
    let store = store();
    let first = store
        .create_view(&new_view("Factures a payer", tag_filter("facture")))
        .expect("vue 1");
    let recent_filter = FilterExpr::Not {
        item: Box::new(FilterExpr::Tag {
            tag: "archive".to_string(),
            negated: false,
        }),
    };
    let second = store
        .create_view(&NewSavedView {
            name: "Recents".to_string(),
            filter: recent_filter.clone(),
            icon: Some("clock".to_string()),
            color: Some(TagColor::Blue),
            position: 0,
            sort: Sort {
                field: SortField::Name,
                dir: SortDir::Desc,
            },
        })
        .expect("vue 2");
    assert_eq!((first.position, second.position), (0, 1));
    assert_eq!(second.color, Some(TagColor::Blue));
    assert_eq!(second.sort.field, SortField::Name);
    assert_eq!(second.filter, recent_filter);

    let fetched = store
        .get_view(second.id)
        .expect("lecture")
        .expect("present");
    assert_eq!(fetched.filter, second.filter);
    assert_eq!(fetched.icon.as_deref(), Some("clock"));

    assert_eq!(
        store
            .get_view_by_name("Recents")
            .expect("nom")
            .map(|view| view.id),
        Some(second.id)
    );
    assert!(
        store
            .get_view_by_name("recents")
            .expect("casse exacte")
            .is_none()
    );

    let listed: Vec<String> = store
        .list_views()
        .expect("liste")
        .into_iter()
        .map(|view| view.name)
        .collect();
    assert_eq!(listed, vec!["Factures a payer", "Recents"]);

    let renamed = store
        .update_view(
            first.id,
            &SavedViewPatch {
                name: Some("  Factures  ".to_string()),
                ..SavedViewPatch::default()
            },
        )
        .expect("renommage");
    assert_eq!(renamed.name, "Factures");
    assert!(renamed.updated_at >= renamed.created_at);

    let filtered = store
        .update_view(
            first.id,
            &SavedViewPatch {
                filter: Some(tag_filter("paye")),
                ..SavedViewPatch::default()
            },
        )
        .expect("filtre");
    assert_eq!(filtered.filter, tag_filter("paye"));

    assert!(matches!(
        store.update_view(
            first.id,
            &SavedViewPatch {
                filter: Some(FilterExpr::And { items: vec![] }),
                ..SavedViewPatch::default()
            }
        ),
        Err(StoreError::Invalid(_))
    ));
    assert!(matches!(
        store.create_view(&new_view("", tag_filter("x"))),
        Err(StoreError::Invalid(_))
    ));
    assert!(matches!(
        store.create_view(&new_view(&"a".repeat(101), tag_filter("x"))),
        Err(StoreError::Invalid(_))
    ));
    assert!(matches!(
        store.create_view(&new_view("Factures", tag_filter("x"))),
        Err(StoreError::Conflict(_))
    ));
    assert!(matches!(
        store.update_view(fs_ez_core::ViewId(999), &SavedViewPatch::default()),
        Err(StoreError::NotFound("view"))
    ));

    assert!(store.delete_view(first.id).expect("suppression"));
    assert!(store.get_view(first.id).expect("lecture").is_none());
    assert!(!store.delete_view(first.id).expect("deja supprime"));
}

#[test]
fn deleting_tag_prunes_saved_view_filters() {
    let store = store();
    let obsolete = tag(&store, "obsolete");
    tag(&store, "keep");
    let mixed = store
        .create_view(&new_view(
            "Mixte",
            FilterExpr::And {
                items: vec![tag_filter("obsolete"), tag_filter("keep")],
            },
        ))
        .expect("vue mixte");
    let single = store
        .create_view(&new_view("Obsolete", tag_filter("obsolete")))
        .expect("vue obsolete");
    let untouched = store
        .create_view(&new_view("Keep", tag_filter("keep")))
        .expect("vue keep");

    assert!(store.delete_tag(obsolete).expect("suppression"));

    assert_eq!(
        store
            .get_view(mixed.id)
            .expect("lecture")
            .expect("presente")
            .filter,
        tag_filter("keep")
    );
    assert!(store.get_view(single.id).expect("lecture").is_none());
    assert!(store.get_view(untouched.id).expect("lecture").is_some());
}

#[test]
fn views_store_sort_text_and_reject_corrupt_filter() {
    let dir = tempfile::tempdir().expect("repertoire");
    let path = dir.path().join("fs-ez.db");
    let store = SqliteStore::open(&path).expect("store");
    let view = store
        .create_view(&NewSavedView {
            name: "Tri par nom".to_string(),
            filter: tag_filter("facture"),
            icon: None,
            color: None,
            position: 0,
            sort: Sort {
                field: SortField::Name,
                dir: SortDir::Desc,
            },
        })
        .expect("vue");

    let seeding = db::open(&path).expect("connexion de seed");
    let (sort, filter_json): (String, String) = seeding
        .query_row(
            "SELECT sort, filter_json FROM saved_view WHERE id = ?1",
            [view.id.0],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("ligne");
    assert_eq!(sort, "name:desc");
    let parsed: FilterExpr = serde_json::from_str(&filter_json).expect("filtre JSON");
    assert_eq!(parsed, tag_filter("facture"));

    seeding
        .execute(
            "UPDATE saved_view SET filter_json = 'not json' WHERE id = ?1",
            [view.id.0],
        )
        .expect("corruption");
    drop(seeding);
    assert!(matches!(
        store.get_view(view.id),
        Err(StoreError::Db(message)) if message == format!("corrupt filter_json for view {}", view.id.0)
    ));
}

#[test]
fn view_group_and_member_reorder_ties_break_by_view_id() {
    let dir = tempfile::tempdir().expect("repertoire");
    let path = dir.path().join("fs-ez.db");
    let store = SqliteStore::open(&path).expect("store");
    let work = store
        .create_view_group("Travail", None)
        .expect("groupe travail");
    let other = store
        .create_view_group("Maison", None)
        .expect("groupe maison");
    let first = store
        .create_view(&new_view("Premiere", tag_filter("a")))
        .expect("vue 1");
    let second = store
        .create_view(&new_view("Seconde", tag_filter("b")))
        .expect("vue 2");
    store.place_view(work.id, second.id, None).expect("place 2");
    store.place_view(work.id, first.id, None).expect("place 1");

    let seeding = db::open(&path).expect("seed");
    seeding
        .execute("UPDATE view_group_member SET position = 0", [])
        .expect("positions egales");
    drop(seeding);
    let names = |store: &SqliteStore| -> Vec<String> {
        store
            .views_in_group(work.id)
            .expect("vues")
            .into_iter()
            .map(|view| view.name)
            .collect()
    };
    assert_eq!(names(&store), vec!["Premiere", "Seconde"]);

    store
        .reorder_group_views(work.id, &[second.id, first.id])
        .expect("ordre vues");
    assert_eq!(names(&store), vec!["Seconde", "Premiere"]);

    store
        .reorder_view_groups(&[other.id, work.id])
        .expect("ordre groupes");
    let listed: Vec<(String, i64)> = store
        .list_view_groups()
        .expect("groupes")
        .into_iter()
        .map(|group| (group.name, group.position))
        .collect();
    assert_eq!(
        listed,
        vec![("Maison".to_string(), 0), ("Travail".to_string(), 1)]
    );
}

#[test]
fn view_groups_scope_reorder_and_move_views() {
    let store = store();
    let work = store
        .create_view_group("Travail", Some(TagColor::Blue))
        .expect("groupe travail");
    let other = store
        .create_view_group("Autre", None)
        .expect("groupe autre");
    let first = store
        .create_view(&new_view("Premiere", tag_filter("a")))
        .expect("vue 1");
    let second = store
        .create_view(&new_view("Seconde", tag_filter("b")))
        .expect("vue 2");

    store.place_view(work.id, first.id, None).expect("place 1");
    store
        .place_view(work.id, second.id, Some(first.id))
        .expect("place 2");
    assert_eq!(
        store.group_for_view(first.id).expect("groupe"),
        Some(work.id)
    );
    let names: Vec<String> = store
        .views_in_group(work.id)
        .expect("vues")
        .into_iter()
        .map(|view| view.name)
        .collect();
    assert_eq!(names, vec!["Seconde", "Premiere"]);

    store
        .place_view(other.id, first.id, None)
        .expect("deplacement");
    assert_eq!(
        store.group_for_view(first.id).expect("groupe"),
        Some(other.id)
    );
    assert!(
        store
            .views_in_group(work.id)
            .expect("travail")
            .iter()
            .all(|view| view.id != first.id)
    );

    let renamed = store
        .update_view_group(
            work.id,
            Some("Travail 2"),
            Some(Some(TagColor::Amber)),
            Some(4),
            Some(true),
        )
        .expect("mise a jour");
    assert_eq!(renamed.name, "Travail 2");
    assert_eq!(renamed.color, Some(TagColor::Amber));
    assert_eq!(renamed.position, 4);
    assert!(renamed.collapsed);

    assert!(store.delete_view_group(other.id).expect("suppression"));
    assert!(store.get_view(first.id).expect("vue").is_none());
    assert!(store.get_view_group(other.id).expect("groupe").is_none());
    assert!(!store.delete_view_group(other.id).expect("deja supprime"));
}
