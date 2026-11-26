use super::*;

fn params() -> FilesParams {
    FilesParams::default()
}

fn valid(query: &str) -> ValidatedFiles {
    FilesParams::from_query(Some(query))
        .expect("params")
        .validate()
        .expect("valide")
}

#[test]
fn trims_and_bounds_search_text() {
    let ok = FilesParams {
        q: Some("  rapport  ".to_string()),
        ..params()
    };
    assert_eq!(ok.validate().expect("valide").q.as_deref(), Some("rapport"));

    let empty = FilesParams {
        q: Some("   ".to_string()),
        ..params()
    };
    assert!(empty.validate().expect("valide").q.is_none());

    let too_long = FilesParams {
        q: Some("a".repeat(fs_ez_store::MAX_TEXT_LEN + 1)),
        ..params()
    };
    assert!(matches!(too_long.validate(), Err(WebError::BadRequest(_))));

    for bad in ["a\nb", "__proto__", "a\u{2028}b"] {
        let value = FilesParams {
            q: Some(bad.to_string()),
            ..params()
        };
        assert!(
            matches!(value.validate(), Err(WebError::BadRequest(_))),
            "{bad:?}"
        );
    }
}

#[test]
fn rejects_unknown_filter_and_sort() {
    for raw in ["bogus", "untagged,recent"] {
        let value = FilesParams {
            filter: Some(raw.to_string()),
            ..params()
        };
        assert!(matches!(value.validate(), Err(WebError::BadRequest(_))));
    }
    let sort = FilesParams {
        sort: Some("date:up".to_string()),
        ..params()
    };
    assert!(matches!(sort.validate(), Err(WebError::BadRequest(_))));
}

#[test]
fn accepts_tag_names_without_query_syntax() {
    let ok = FilesParams {
        tags: vec!["  Facture ".to_string()],
        ..params()
    };
    assert_eq!(
        ok.validate().expect("valide").tags,
        vec!["Facture".to_string()]
    );

    let invalid = FilesParams {
        tags: vec!["a=b".to_string()],
        ..params()
    };
    assert!(matches!(invalid.validate(), Err(WebError::BadRequest(_))));
}

#[test]
fn parses_repeated_tags_and_empty_tag_clears_them() {
    let repeated = FilesParams::from_query(Some("q=rapport&tag=Facture&tag=2025")).expect("params");
    assert_eq!(repeated.q.as_deref(), Some("rapport"));
    assert_eq!(repeated.tags, vec!["Facture", "2025"]);

    let cleared = FilesParams::from_query(Some("tag=Facture&tag=&tag=2025")).expect("params");
    assert!(cleared.tags.is_empty());

    let validated = FilesParams {
        tags: vec![
            "Facture".to_string(),
            "facture".to_string(),
            "2025".to_string(),
        ],
        ..params()
    };
    assert_eq!(
        validated.validate().expect("valide").tags,
        vec!["Facture".to_string(), "2025".to_string()]
    );
}

#[test]
fn parses_excluded_tags_and_keeps_them_separate() {
    let parsed =
        FilesParams::from_query(Some("tag=Facture&not_tag=archive&not_tag=2025")).expect("params");
    assert_eq!(parsed.tags, vec!["Facture"]);
    assert_eq!(parsed.exclude_tags, vec!["archive", "2025"]);

    let cleared = FilesParams::from_query(Some("not_tag=archive&not_tag=")).expect("params");
    assert!(cleared.exclude_tags.is_empty());

    let validated = FilesParams {
        exclude_tags: vec!["Archive".to_string(), "archive".to_string()],
        ..params()
    };
    let validated = validated.validate().expect("valide");
    assert_eq!(validated.exclude_tags, vec!["Archive".to_string()]);
    assert!(validated.tags.is_empty());
}

#[test]
fn untagged_rejects_excluded_tags() {
    let value = FilesParams {
        filter: Some("untagged".to_string()),
        exclude_tags: vec!["archive".to_string()],
        ..params()
    };
    assert!(matches!(value.validate(), Err(WebError::BadRequest(_))));
}

#[test]
fn sort_defaults_to_ascending_outside_strict_codec() {
    let sort = FilesParams {
        sort: Some("name".to_string()),
        ..params()
    };
    assert_eq!(
        sort.validate().expect("valide").sort,
        Some(Sort {
            field: SortField::Name,
            dir: SortDir::Asc,
        })
    );

    let sort = FilesParams {
        sort: Some("name:desc".to_string()),
        ..params()
    };
    assert_eq!(
        sort.validate().expect("valide").sort,
        Some(Sort {
            field: SortField::Name,
            dir: SortDir::Desc,
        })
    );

    let sort = FilesParams {
        sort: Some("bogus".to_string()),
        ..params()
    };
    assert!(matches!(sort.validate(), Err(WebError::BadRequest(_))));
}

#[test]
fn sort_tokens_round_trip() {
    for field in [
        SortField::Id,
        SortField::Name,
        SortField::Size,
        SortField::Mtime,
    ] {
        for dir in [SortDir::Asc, SortDir::Desc] {
            let sort = Sort { field, dir };
            assert_eq!(parse_sort(&sort_token(sort)), Some(sort));
        }
    }
}

#[test]
fn virtual_folder_accepts_only_positive_ids() {
    for raw in ["vf=0", "vf=-3", "vf=abc"] {
        let parsed = FilesParams::from_query(Some(raw)).expect("params");
        assert!(
            matches!(parsed.validate(), Err(WebError::BadRequest(_))),
            "{raw:?}"
        );
    }
    assert_eq!(
        valid("vf=root").virtual_folder,
        Some(VirtualFolderScope::Root)
    );
    assert_eq!(
        valid("vf=42").virtual_folder,
        Some(VirtualFolderScope::Folder(VirtualFolderId(42)))
    );
}

#[test]
fn scope_url_decodes_view_and_query() {
    let params = FilesParams::from_scope_url("/views/7?q=rapport&sort=name").expect("scope");
    assert_eq!(params.view, Some(ViewId(7)));
    assert_eq!(params.q.as_deref(), Some("rapport"));
    let valid = params.validate().expect("valide");
    assert_eq!(valid.view, Some(ViewId(7)));
    assert_eq!(
        valid.sort,
        Some(Sort {
            field: SortField::Name,
            dir: SortDir::Asc,
        })
    );
}

#[test]
fn scope_url_decodes_virtual_folders() {
    assert_eq!(
        FilesParams::from_scope_url("/virtual-folders")
            .expect("racine")
            .validate()
            .expect("valide")
            .virtual_folder,
        Some(VirtualFolderScope::Root)
    );
    assert_eq!(
        FilesParams::from_scope_url("/virtual-folders/5?tag=Facture")
            .expect("dossier")
            .validate()
            .expect("valide")
            .virtual_folder,
        Some(VirtualFolderScope::Folder(VirtualFolderId(5)))
    );
}

#[test]
fn scope_url_rejects_invalid_paths() {
    assert!(matches!(
        FilesParams::from_scope_url("/views/abc"),
        Err(WebError::BadRequest(_))
    ));
    assert!(matches!(
        FilesParams::from_scope_url("/virtual-folders/-1"),
        Err(WebError::BadRequest(_))
    ));
    assert!(matches!(
        FilesParams::from_scope_url("/elsewhere"),
        Err(WebError::BadRequest(_))
    ));
}

#[test]
fn virtual_folder_token_is_stable() {
    assert_eq!(
        valid("vf=root").virtual_folder_token().as_deref(),
        Some("root")
    );
    assert_eq!(valid("vf=7").virtual_folder_token().as_deref(), Some("7"));
    assert_eq!(
        params().validate().expect("valide").virtual_folder_token(),
        None
    );
}
