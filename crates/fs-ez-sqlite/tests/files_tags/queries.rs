use super::*;

#[test]
fn offset_pagination_covers_all_sorts_without_loss() {
    let store = store();
    let day1 = datetime!(2026-01-01 00:00 UTC);
    let day2 = datetime!(2026-01-02 00:00 UTC);
    let day3 = datetime!(2026-01-03 00:00 UTC);
    insert(&store, "/x/a", None, 30, day1);
    insert(&store, "/x/b", None, 10, day2);
    insert(&store, "/y/b", None, 20, day2);
    insert(&store, "/x/c", None, 10, day3);
    insert(&store, "/x/d", None, 40, day1);

    let cases = [
        (SortField::Id, SortDir::Asc, vec![1, 2, 3, 4, 5]),
        (SortField::Id, SortDir::Desc, vec![5, 4, 3, 2, 1]),
        (SortField::Name, SortDir::Asc, vec![1, 2, 3, 4, 5]),
        (SortField::Name, SortDir::Desc, vec![5, 4, 3, 2, 1]),
        (SortField::Size, SortDir::Asc, vec![2, 4, 3, 1, 5]),
        (SortField::Size, SortDir::Desc, vec![5, 1, 3, 4, 2]),
        (SortField::Mtime, SortDir::Asc, vec![1, 5, 2, 3, 4]),
        (SortField::Mtime, SortDir::Desc, vec![4, 3, 2, 5, 1]),
    ];
    for (field, dir, expected) in cases {
        let ids = collect_ids(&store, sort(field, dir));
        assert_eq!(ids, expected, "{field:?} {dir:?}");
        assert_eq!(ids.iter().copied().collect::<HashSet<_>>().len(), 5);
    }
}

#[test]
fn offset_windows_are_contiguous_and_count_ignores_pagination() {
    let store = store();
    let at = datetime!(2026-01-01 00:00 UTC);
    for index in 1..=5 {
        insert(&store, &format!("/f-{index}"), None, index, at);
    }
    let ids = |page: &fs_ez_store::Page<fs_ez_core::FileWithTags>| {
        page.items
            .iter()
            .map(|item| item.file.id.0)
            .collect::<Vec<_>>()
    };
    let base = query(sort(SortField::Id, SortDir::Asc), 0, 2);
    let first = store.list_files(&base).expect("premiere page");
    assert_eq!(ids(&first), vec![1, 2]);
    assert_eq!(first.next_offset, Some(2));

    let second = store
        .list_files(&FileListQuery {
            offset: 2,
            ..base.clone()
        })
        .expect("deuxieme page");
    assert_eq!(ids(&second), vec![3, 4]);
    assert_eq!(second.next_offset, Some(4));

    let third = store
        .list_files(&FileListQuery {
            offset: 4,
            ..base.clone()
        })
        .expect("derniere page");
    assert_eq!(ids(&third), vec![5]);
    assert_eq!(third.next_offset, None);

    assert_eq!(store.count_files_query(&base).expect("compte"), 5);
    let filtered = FileListQuery {
        text: Some("f-3".to_string()),
        ..base
    };
    assert_eq!(
        store.count_files_query(&filtered).expect("compte filtre"),
        1
    );
    let filtered_page = store
        .list_files(&FileListQuery {
            offset: 1,
            ..filtered
        })
        .expect("page filtree");
    assert!(filtered_page.items.is_empty());
}

#[test]
fn untagged_and_tagged_since_filter_queries() {
    let store = store();
    let now = OffsetDateTime::now_utc();
    let old = now - Duration::days(30);
    let first = insert(&store, "/a", None, 0, now);
    let second = insert(&store, "/b", None, 0, now);
    insert(&store, "/c", None, 0, now);
    let tag = tag(&store, "recent");
    store.apply_tag(first, tag, true, now).expect("recent");
    store.apply_tag(second, tag, true, old).expect("ancien");

    assert_eq!(store.count_untagged().expect("sans tag"), 1);
    let untagged = store
        .list_files(&FileListQuery {
            untagged: true,
            ..query(sort(SortField::Id, SortDir::Asc), 0, 10)
        })
        .expect("liste sans tag");
    assert_eq!(untagged.items.len(), 1);
    assert_eq!(untagged.items[0].file.path, PathBuf::from("/c"));

    assert_eq!(store.count_tagged_since(7).expect("recent"), 1);
    let recent = store
        .list_files(&FileListQuery {
            tagged_since_days: Some(7),
            ..query(sort(SortField::Id, SortDir::Asc), 0, 10)
        })
        .expect("liste recente");
    assert_eq!(recent.items.len(), 1);
    assert_eq!(recent.items[0].file.id, first);

    assert!(matches!(
        store.list_files(&FileListQuery {
            untagged: true,
            tagged_since_days: Some(7),
            ..query(sort(SortField::Id, SortDir::Asc), 0, 10)
        }),
        Err(StoreError::Invalid(_))
    ));
    assert!(matches!(
        store.list_files(&query(sort(SortField::Id, SortDir::Asc), 0, 0)),
        Err(StoreError::Invalid(_))
    ));
    assert!(matches!(
        store.list_files(&query(sort(SortField::Id, SortDir::Asc), 0, 501)),
        Err(StoreError::Invalid(_))
    ));
}

#[test]
fn recent_lists_most_recently_tagged_first() {
    let store = store();
    let now = OffsetDateTime::now_utc();
    let old = now - Duration::days(2);
    let middle = now - Duration::days(1);
    let first = insert(&store, "/a", None, 0, now);
    let second = insert(&store, "/b", None, 0, now);
    let third = insert(&store, "/c", None, 0, now);
    let tag = tag(&store, "recent-order");
    for (file, at) in [(first, old), (second, now), (third, middle)] {
        store.apply_tag(file, tag, true, at).expect("tag");
    }

    let recent = store
        .list_files(&FileListQuery {
            tagged_since_days: Some(7),
            ..query(sort(SortField::Id, SortDir::Asc), 0, 10)
        })
        .expect("liste recente");
    assert_eq!(
        recent
            .items
            .iter()
            .map(|item| item.file.id)
            .collect::<Vec<_>>(),
        vec![second, third, first]
    );
}

#[test]
fn recent_files_filter_uses_mtime_and_lists_newest_first() {
    let store = store();
    let now = OffsetDateTime::now_utc();
    let first = insert(&store, "/old", None, 0, now - Duration::days(30));
    let second = insert(&store, "/new", None, 0, now);
    let third = insert(&store, "/middle", None, 0, now - Duration::days(1));

    let recent = store
        .list_files(&FileListQuery {
            modified_since_days: Some(7),
            ..query(sort(SortField::Id, SortDir::Asc), 0, 10)
        })
        .expect("fichiers recents");
    assert_eq!(
        recent
            .items
            .iter()
            .map(|item| item.file.id)
            .collect::<Vec<_>>(),
        vec![second, third]
    );
    assert_eq!(
        store
            .count_files_query(&FileListQuery {
                modified_since_days: Some(7),
                ..query(sort(SortField::Id, SortDir::Asc), 0, 10)
            })
            .expect("compte"),
        2
    );
    assert!(!recent.items.iter().any(|item| item.file.id == first));
}

#[test]
fn text_search_is_case_insensitive_and_escapes_wildcards() {
    let store = store();
    let at = datetime!(2026-01-01 00:00 UTC);
    insert(&store, "/data/Rapport-annuel.txt", None, 1, at);
    insert(&store, "/data/100%-couvert.txt", None, 1, at);
    insert(&store, "/data/a_b.txt", None, 1, at);
    insert(&store, "/data/ab.txt", None, 1, at);
    insert(&store, "/data/back\\slash.txt", None, 1, at);

    let search = |text: &str| {
        store
            .list_files(&FileListQuery {
                text: Some(text.to_string()),
                ..query(sort(SortField::Name, SortDir::Asc), 0, 10)
            })
            .expect("recherche")
            .items
            .into_iter()
            .map(|item| item.file.name)
            .collect::<Vec<_>>()
    };

    assert_eq!(search("RAPPORT"), vec!["Rapport-annuel.txt"]);
    assert_eq!(search("%"), vec!["100%-couvert.txt"]);
    assert_eq!(search("_"), vec!["a_b.txt"]);
    assert_eq!(search("\\"), vec!["back\\slash.txt"]);
    assert_eq!(search("ab"), vec!["ab.txt"]);
}
