use super::*;

#[test]
fn tag_counts_are_distinct_per_file() {
    let store = store();
    let at = datetime!(2026-01-01 00:00 UTC);
    let first = insert(&store, "/a", None, 0, at);
    let second = insert(&store, "/b", None, 0, at);
    let alpha = tag(&store, "alpha");
    let beta = tag(&store, "beta");
    let empty = tag(&store, "vide");
    store.apply_tag(first, alpha, true, at).expect("alpha");
    store.apply_tag(second, alpha, true, at).expect("alpha");
    store.apply_tag(first, beta, true, at).expect("beta");
    store
        .apply_tag(first, beta, true, at)
        .expect("beta idempotent");
    let counts = store.tag_counts().expect("compteurs");
    let by_id: Vec<(TagId, u64)> = counts
        .iter()
        .map(|count| (count.tag.id, count.count))
        .collect();
    assert_eq!(by_id, vec![(alpha, 2), (beta, 1), (empty, 0)]);
}
