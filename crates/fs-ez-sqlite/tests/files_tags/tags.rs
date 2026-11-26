use super::*;

#[test]
fn apply_tag_promotes_implicit_and_removal_is_idempotent() {
    let store = store();
    let at = datetime!(2026-01-01 00:00 UTC);
    let file = insert(&store, "/a", None, 0, at);
    let tag = tag(&store, "projet");

    assert!(store.apply_tag(file, tag, true, at).expect("application"));
    assert!(!store.apply_tag(file, tag, true, at).expect("idempotent"));
    assert_eq!(store.tags_for_file(file).expect("tags").len(), 1);

    let other = insert(&store, "/b", None, 0, at);
    assert!(store.apply_tag(other, tag, false, at).expect("implicite"));
    assert!(store.apply_tag(other, tag, true, at).expect("promotion"));
    let applied = store.tags_for_file(other).expect("tags");
    assert_eq!(applied.len(), 1);
    assert!(applied[0].explicit);
    assert!(
        !store
            .apply_tag(other, tag, false, at)
            .expect("deja explicite")
    );
    assert!(store.tags_for_file(other).expect("tags")[0].explicit);

    assert!(store.remove_tag(file, tag).expect("retrait"));
    assert!(!store.remove_tag(file, tag).expect("deja retire"));
    assert!(store.tags_for_file(file).expect("tags").is_empty());
}

#[test]
fn tags_for_file_returns_one_entry_per_tag() {
    let store = store();
    let at = datetime!(2026-01-01 00:00 UTC);
    let file = insert(&store, "/a", None, 0, at);
    let alpha = tag(&store, "alpha");
    let beta = tag(&store, "beta");
    store.apply_tag(file, alpha, true, at).expect("alpha");
    store.apply_tag(file, beta, true, at).expect("beta");
    let applied = store.tags_for_file(file).expect("tags");
    assert_eq!(applied.len(), 2);
    assert_eq!(applied[0].tag.name, "alpha");
    assert_eq!(applied[1].tag.name, "beta");
}

#[test]
fn tag_normalization_rename_conflict_and_colors() {
    let store = store();
    let group = store.create_group("Tests", None).expect("groupe");
    let created = store
        .get_or_create_tag(group.id, "  Projet  ", TagColor::Blue)
        .expect("creation");
    assert_eq!(created.name, "Projet");
    assert_eq!(created.name_norm, "projet");
    let again = store
        .get_or_create_tag(group.id, "projet", TagColor::Amber)
        .expect("existante");
    assert_eq!(again.id, created.id);
    assert_eq!(again.color, TagColor::Blue);
    assert_eq!(
        store
            .get_tag_by_name("PROJET")
            .expect("recherche")
            .expect("trouve")
            .id,
        created.id
    );

    let renamed = store.rename_tag(created.id, "Archive").expect("renommage");
    assert_eq!(renamed.name, "Archive");
    assert!(matches!(
        store.rename_tag(created.id, "a=b"),
        Err(StoreError::Invalid(_))
    ));

    let other = tag(&store, "autre");
    assert!(matches!(
        store.rename_tag(other, "archive"),
        Err(StoreError::Conflict(_))
    ));

    let colored = store
        .set_tag_color(created.id, TagColor::Red, Some("#AABBCC".to_string()))
        .expect("couleur");
    assert_eq!(colored.color, TagColor::Red);
    assert_eq!(colored.color_custom.as_deref(), Some("#aabbcc"));
    let cleared = store
        .set_tag_color(created.id, TagColor::Green, None)
        .expect("efface");
    assert_eq!(cleared.color, TagColor::Green);
    assert!(cleared.color_custom.is_none());
    assert!(matches!(
        store.set_tag_color(created.id, TagColor::Blue, Some("bleu".to_string())),
        Err(StoreError::Invalid(_))
    ));

    let names: Vec<String> = store
        .list_tags()
        .expect("liste")
        .into_iter()
        .map(|tag| tag.name)
        .collect();
    assert_eq!(names, vec!["Archive", "autre"]);
}
