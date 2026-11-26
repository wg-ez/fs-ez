use super::*;

use fs_ez_store::FileStore;

#[test]
fn groups_positions_rename_color_and_collapse() {
    let store = store();
    store.create_group("Travail", None).expect("groupe 1");
    let second = store
        .create_group("  Maison  ", Some(TagColor::Green))
        .expect("groupe 2");
    let third = store.create_group("Divers", None).expect("groupe 3");
    let listed: Vec<(String, i64)> = store
        .list_groups()
        .expect("liste")
        .into_iter()
        .map(|group| (group.name, group.position))
        .collect();
    assert_eq!(
        listed,
        vec![
            ("Divers".to_string(), 0),
            ("Maison".to_string(), 1),
            ("Travail".to_string(), 2),
        ]
    );
    assert_eq!(second.name, "Maison");
    assert_eq!(second.color, Some(TagColor::Green));
    assert!(!third.collapsed);

    let updated = store
        .update_group(
            second.id,
            Some("Maison et jardin"),
            Some(Some(TagColor::Amber)),
            Some(9),
            Some(true),
        )
        .expect("mise a jour");
    assert_eq!(updated.name, "Maison et jardin");
    assert_eq!(updated.color, Some(TagColor::Amber));
    assert_eq!(updated.position, 9);
    assert!(updated.collapsed);

    let cleared = store
        .update_group(second.id, None, Some(None), None, Some(false))
        .expect("couleur effacee");
    assert!(cleared.color.is_none());
    assert!(!cleared.collapsed);
    assert_eq!(cleared.name, "Maison et jardin");

    assert!(matches!(
        store.create_group("   ", None),
        Err(StoreError::Invalid(_))
    ));
    assert!(matches!(
        store.create_group(&"a".repeat(65), None),
        Err(StoreError::Invalid(_))
    ));
    assert!(matches!(
        store.create_group("a\nb", None),
        Err(StoreError::Invalid(_))
    ));
    assert!(matches!(
        store.create_group("Travail", None),
        Err(StoreError::Conflict(_))
    ));
    assert!(matches!(
        store.update_group(third.id, Some("Travail"), None, None, None),
        Err(StoreError::Conflict(_))
    ));
    assert!(matches!(
        store.update_group(fs_ez_core::GroupId(999), None, None, None, Some(true)),
        Err(StoreError::NotFound("group"))
    ));
}

#[test]
fn tag_belongs_to_one_group_and_moves_atomically() {
    let store = store();
    let work = store.create_group("Travail", None).expect("travail");
    let other = store.create_group("Autre", None).expect("autre");
    let alpha = store
        .get_or_create_tag(work.id, "alpha", TagColor::Neutral)
        .expect("alpha")
        .id;
    let beta = store
        .get_or_create_tag(work.id, "beta", TagColor::Neutral)
        .expect("beta")
        .id;

    assert_eq!(store.group_for_tag(alpha).expect("groupe"), Some(work.id));
    assert_eq!(store.group_for_tag(beta).expect("groupe"), Some(work.id));

    store
        .place_tag(work.id, beta, Some(alpha))
        .expect("reordonne");
    let work_tags: Vec<String> = store
        .tags_in_group(work.id)
        .expect("tags travail")
        .into_iter()
        .map(|tag| tag.name)
        .collect();
    assert_eq!(work_tags, vec!["beta", "alpha"]);

    store.place_tag(other.id, alpha, None).expect("deplacement");
    assert_eq!(store.group_for_tag(alpha).expect("groupe"), Some(other.id));
    assert!(
        store
            .tags_in_group(work.id)
            .expect("travail")
            .iter()
            .all(|tag| tag.id != alpha)
    );
    assert_eq!(store.tags_in_group(other.id).expect("autre").len(), 1);
    assert_eq!(
        store.get_tag(alpha).expect("tag").expect("present").id,
        alpha
    );
}

#[test]
fn group_and_member_reorder_follow_given_order_and_ties_break_by_name() {
    let dir = tempfile::tempdir().expect("repertoire");
    let path = dir.path().join("fs-ez.db");
    let store = SqliteStore::open(&path).expect("store");
    let work = store.create_group("Travail", None).expect("travail");
    let home = store.create_group("Maison", None).expect("maison");
    let zeta = store
        .get_or_create_tag(work.id, "Zeta", TagColor::Neutral)
        .expect("zeta")
        .id;
    let alpha = store
        .get_or_create_tag(work.id, "alpha", TagColor::Neutral)
        .expect("alpha")
        .id;

    let seeding = db::open(&path).expect("seed");
    seeding
        .execute("UPDATE tag_group_member SET position = 0", [])
        .expect("positions egales");
    drop(seeding);
    let names = |store: &SqliteStore| -> Vec<String> {
        store
            .tags_in_group(work.id)
            .expect("tags")
            .into_iter()
            .map(|tag| tag.name)
            .collect()
    };
    assert_eq!(names(&store), vec!["alpha", "Zeta"]);

    store
        .reorder_group_tags(work.id, &[zeta, alpha])
        .expect("ordre tags");
    assert_eq!(names(&store), vec!["Zeta", "alpha"]);

    store
        .reorder_groups(&[home.id, work.id])
        .expect("ordre groupes");
    let listed: Vec<(String, i64)> = store
        .list_groups()
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
fn deleting_a_group_cascades_to_file_tag() {
    let store = store();
    let group = store.create_group("Travail", None).expect("groupe");
    let alpha = store
        .get_or_create_tag(group.id, "alpha", TagColor::Neutral)
        .expect("alpha")
        .id;
    let now = time::OffsetDateTime::now_utc();
    let file = super::common::insert(&store, "/cascade/a.txt", None, 1, now);
    store
        .apply_tag(file, alpha, true, now)
        .expect("application");
    assert_eq!(store.tags_for_file(file).expect("tags").len(), 1);

    assert!(store.delete_group(group.id).expect("suppression"));
    assert!(store.tags_for_file(file).expect("tags").is_empty());
}

#[test]
fn deleting_a_group_deletes_its_tags() {
    let store = store();
    let group = store.create_group("Travail", None).expect("groupe");
    let alpha = store
        .get_or_create_tag(group.id, "alpha", TagColor::Neutral)
        .expect("alpha")
        .id;

    assert!(store.delete_group(group.id).expect("suppression"));
    assert!(store.get_group(group.id).expect("lecture").is_none());
    assert!(store.get_tag(alpha).expect("tag").is_none());
    assert!(store.group_for_tag(alpha).expect("groupe").is_none());
    assert!(!store.delete_group(group.id).expect("deja supprime"));
}
