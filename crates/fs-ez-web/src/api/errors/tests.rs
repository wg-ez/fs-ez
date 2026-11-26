use super::*;

#[test]
fn limits_are_bounded() {
    assert_eq!(parse_limit(None).expect("défaut"), 100);
    assert_eq!(parse_limit(Some("1")).expect("min"), 1);
    assert_eq!(parse_limit(Some("500")).expect("max"), 500);
    for raw in ["0", "501", "-1", "abc", "1.5"] {
        assert!(matches!(
            parse_limit(Some(raw)),
            Err(WebError::BadRequest(_))
        ));
    }
}

#[test]
fn ids_must_be_positive_integers() {
    assert_eq!(parse_id("12").expect("valide"), 12);
    for raw in ["0", "-3", "abc", ""] {
        assert!(matches!(parse_id(raw), Err(WebError::BadRequest(_))));
    }
}

#[test]
fn sorts_default_to_ascending() {
    let sort = parse_sort(Some("mtime")).expect("valide").expect("présent");
    assert_eq!(sort.field, fs_ez_core::SortField::Mtime);
    assert_eq!(sort.dir, fs_ez_core::SortDir::Asc);
    let sort = parse_sort(Some("name:desc"))
        .expect("valide")
        .expect("présent");
    assert_eq!(sort.dir, fs_ez_core::SortDir::Desc);
    for raw in ["date", "name:up", "name:desc:extra"] {
        assert!(matches!(
            parse_sort(Some(raw)),
            Err(WebError::BadRequest(_))
        ));
    }
}
