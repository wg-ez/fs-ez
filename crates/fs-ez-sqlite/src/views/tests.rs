use fs_ez_core::{Sort, SortDir, SortField};
use fs_ez_store::StoreError;

#[test]
fn sort_encoding_round_trip() {
    for field in [
        SortField::Id,
        SortField::Name,
        SortField::Size,
        SortField::Mtime,
    ] {
        for dir in [SortDir::Asc, SortDir::Desc] {
            let sort = Sort { field, dir };
            assert_eq!(Sort::decode(&sort.encode()).expect("tri valide"), sort);
        }
    }
    assert_eq!(
        Sort {
            field: SortField::Name,
            dir: SortDir::Desc,
        }
        .encode(),
        "name:desc"
    );
}

#[test]
fn sort_decode_rejects_unknown_values() {
    for raw in [
        "",
        "id",
        "id:",
        ":asc",
        "id:up",
        "date:asc",
        "id:asc:extra",
        "ID:asc",
        "id:ASC",
    ] {
        assert!(Sort::decode(raw).is_none(), "{raw:?}");
    }
}

#[test]
fn next_position_rejects_overflow() {
    let conn = crate::db::open_in_memory().expect("base");
    assert_eq!(super::next_position(&conn).expect("table vide"), 0);
    conn.execute(
        "INSERT INTO saved_view(name, filter_json, position, created_at, updated_at)
         VALUES ('v', '{}', ?1, 'x', 'x')",
        [i64::MAX],
    )
    .expect("insertion");
    assert!(matches!(
        super::next_position(&conn).expect_err("débordement"),
        StoreError::Invalid(message) if message == "position overflow"
    ));
}
