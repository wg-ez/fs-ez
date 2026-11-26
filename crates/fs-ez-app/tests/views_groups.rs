//! Tests d'integration des vues et des groupes côté application.

use fs_ez_app::{AppError, ViewInput};
use fs_ez_core::{FilterExpr, Sort, SortDir, SortField, TagColor};

mod common;

use common::{Harness, create_tag, write};

fn tag_filter(name: &str) -> FilterExpr {
    FilterExpr::Tag {
        tag: name.to_string(),
        negated: false,
    }
}

fn input(name: &str) -> ViewInput {
    ViewInput {
        name: name.to_string(),
        filter: None,
        icon: None,
        color: None,
        position: None,
        sort: Sort::default(),
    }
}

fn name_sort() -> Sort {
    Sort {
        field: SortField::Name,
        dir: SortDir::Desc,
    }
}

#[test]
fn create_view_requires_a_valid_filter_and_name() {
    let harness = Harness::new();
    let app = &harness.app;

    let created = app
        .create_view(ViewInput {
            filter: Some(tag_filter("facture")),
            ..input("Factures")
        })
        .expect("vue filtre");
    assert_eq!(created.filter, tag_filter("facture"));
    assert_eq!(
        app.canonical_filter(&created.filter).expect("canon"),
        "facture"
    );

    assert!(matches!(
        app.create_view(input("Sans filtre")),
        Err(AppError::Invalid(_))
    ));
    assert!(matches!(
        app.create_view(ViewInput {
            filter: Some(tag_filter("x")),
            ..input("   ")
        }),
        Err(AppError::Invalid(_))
    ));
    for bad in ["a\nb", "__proto__", "constructor"] {
        assert!(
            matches!(
                app.create_view(ViewInput {
                    filter: Some(tag_filter("x")),
                    ..input(bad)
                }),
                Err(AppError::Invalid(_))
            ),
            "{bad:?}"
        );
    }
    let long_icon = "a".repeat(17);
    for icon in ["a\nb", "__proto__", long_icon.as_str()] {
        assert!(
            matches!(
                app.create_view(ViewInput {
                    filter: Some(tag_filter("x")),
                    icon: Some(icon.to_string()),
                    ..input("Icône")
                }),
                Err(AppError::Invalid(_))
            ),
            "{icon:?}"
        );
    }
    let view = app
        .create_view(ViewInput {
            filter: Some(tag_filter("x")),
            icon: Some("  ◆  ".to_string()),
            ..input("Icône valide")
        })
        .expect("icône valide");
    assert_eq!(view.icon.as_deref(), Some("◆"));
}

#[test]
fn run_view_paginates_with_view_sort_and_count_is_exact() {
    let harness = Harness::new();
    let app = &harness.app;
    for name in ["a.txt", "b.txt", "c.txt"] {
        let path = write(&harness.temp, name, name.as_bytes());
        app.tag_path(&path, &["x".to_string()], false).expect("tag");
    }
    let view = app
        .create_view(ViewInput {
            filter: Some(tag_filter("x")),
            sort: name_sort(),
            ..input("Tri nom")
        })
        .expect("vue");

    let first = app.run_view(view.id, 0, 2).expect("page 1");
    assert_eq!(
        first
            .items
            .iter()
            .map(|item| item.file.name.as_str())
            .collect::<Vec<_>>(),
        vec!["c.txt", "b.txt"]
    );
    let offset = first.next_offset.expect("offset");
    let second = app.run_view(view.id, offset, 2).expect("page 2");
    assert_eq!(second.items.len(), 1);
    assert_eq!(second.items[0].file.name, "a.txt");
    assert_eq!(second.next_offset, None);
    assert_eq!(app.count_view(view.id).expect("compte"), 3);

    assert!(app.run_view(view.id, 0, 0).is_err());
}

#[test]
fn groups_assign_move_and_delete() {
    let harness = Harness::new();
    let app = &harness.app;
    let work = app.create_group("Travail", None).expect("groupe 1");
    let home = app
        .create_group("Maison", Some(TagColor::Green))
        .expect("groupe 2");
    create_tag(app, "alpha", TagColor::Neutral);

    app.assign_tag(work.id, "alpha").expect("assign 1");
    app.assign_tag(home.id, "alpha").expect("move");
    app.assign_tag(home.id, "alpha").expect("no-op");
    assert!(app.tags_in_group(work.id).expect("tags").is_empty());
    assert_eq!(app.tags_in_group(home.id).expect("tags").len(), 1);

    app.move_tag(home.id, work.id, "alpha")
        .expect("déplacement");
    assert_eq!(app.tags_in_group(work.id).expect("work").len(), 1);
    assert!(app.tags_in_group(home.id).expect("vide").is_empty());
    app.assign_tag(home.id, "alpha").expect("retour");

    let renamed = app.rename_group(home.id, "Maison 2").expect("renommage");
    assert_eq!(renamed.name, "Maison 2");
    let colored = app
        .set_group_color(home.id, Some(TagColor::Blue))
        .expect("couleur");
    assert_eq!(colored.color, Some(TagColor::Blue));
    let collapsed = app.set_group_collapsed(home.id, true).expect("repli");
    assert!(collapsed.collapsed);

    assert!(app.delete_group(home.id).expect("suppression"));
    assert!(app.tags_in_group(home.id).expect("vide").is_empty());
    assert!(app.list_tags().expect("tags").is_empty());

    assert!(matches!(
        app.assign_tag(home.id, "alpha"),
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        app.assign_tag(work.id, "absent"),
        Err(AppError::NotFound(_))
    ));
}

#[test]
fn fallback_groups_create_divers_then_reuse_the_first_group() {
    let harness = Harness::new();
    let app = &harness.app;

    let fallback = app.fallback_group().expect("groupe de repli");
    let groups = app.list_groups().expect("groupes");
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].id, fallback);
    assert_eq!(groups[0].name, "Divers");
    assert_eq!(groups[0].position, 0);
    assert_eq!(app.fallback_group().expect("reutilisation"), fallback);

    let created = app.create_group("Travail", None).expect("groupe");
    assert_eq!(app.fallback_group().expect("premier groupe"), created.id);
}

#[test]
fn fallback_view_groups_create_divers_then_reuse_the_first_group() {
    let harness = Harness::new();
    let app = &harness.app;

    let fallback = app.fallback_view_group().expect("groupe de repli");
    let groups = app.list_view_groups().expect("groupes");
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].id, fallback);
    assert_eq!(groups[0].name, "Divers");
    assert_eq!(groups[0].position, 0);
    assert_eq!(app.fallback_view_group().expect("reutilisation"), fallback);

    let created = app.create_view_group("Travail", None).expect("groupe");
    assert_eq!(
        app.fallback_view_group().expect("premier groupe"),
        created.id
    );
}

#[test]
fn groups_reorder_prepend_and_reorder_members() {
    let harness = Harness::new();
    let app = &harness.app;
    let work = app.create_group("Travail", None).expect("groupe 1");
    let home = app.create_group("Maison", None).expect("groupe 2");

    app.reorder_groups(&[work.id, home.id]).expect("ordre");
    assert_eq!(
        app.list_groups()
            .expect("groupes")
            .into_iter()
            .map(|group| group.name)
            .collect::<Vec<_>>(),
        vec!["Travail", "Maison"]
    );

    let alpha = create_tag(app, "alpha", TagColor::Neutral);
    let zeta = create_tag(app, "zeta", TagColor::Neutral);
    app.prepend_tag(work.id, "zeta").expect("tete");
    assert_eq!(
        app.tags_in_group(work.id)
            .expect("tags")
            .into_iter()
            .map(|tag| tag.name)
            .collect::<Vec<_>>(),
        vec!["zeta", "alpha"]
    );
    app.reorder_group_tags(work.id, &[alpha, zeta])
        .expect("ordre tags");
    assert_eq!(
        app.tags_in_group(work.id)
            .expect("tags")
            .into_iter()
            .map(|tag| tag.name)
            .collect::<Vec<_>>(),
        vec!["alpha", "zeta"]
    );
}

#[test]
fn view_groups_reorder_and_prepend_views() {
    let harness = Harness::new();
    let app = &harness.app;
    let work = app.create_view_group("Travail", None).expect("groupe 1");
    let home = app.create_view_group("Maison", None).expect("groupe 2");

    app.reorder_view_groups(&[home.id, work.id]).expect("ordre");
    assert_eq!(
        app.list_view_groups()
            .expect("groupes")
            .into_iter()
            .map(|group| group.name)
            .collect::<Vec<_>>(),
        vec!["Maison", "Travail"]
    );

    let alpha = app
        .create_view_in_group(
            ViewInput {
                filter: Some(tag_filter("a")),
                ..input("Alpha")
            },
            Some(work.id),
        )
        .expect("vue alpha");
    let zeta = app
        .create_view_in_group(
            ViewInput {
                filter: Some(tag_filter("z")),
                ..input("Zeta")
            },
            Some(work.id),
        )
        .expect("vue zeta");
    app.prepend_view(work.id, alpha.id).expect("tete");
    assert_eq!(
        app.views_in_group(work.id)
            .expect("vues")
            .into_iter()
            .map(|view| view.name)
            .collect::<Vec<_>>(),
        vec!["Alpha", "Zeta"]
    );
    app.reorder_group_views(work.id, &[zeta.id, alpha.id])
        .expect("ordre vues");
    assert_eq!(
        app.views_in_group(work.id)
            .expect("vues")
            .into_iter()
            .map(|view| view.name)
            .collect::<Vec<_>>(),
        vec!["Zeta", "Alpha"]
    );
}

#[test]
fn view_groups_assign_move_and_delete_their_views() {
    let harness = Harness::new();
    let app = &harness.app;
    let work = app
        .create_view_group("Travail", Some(TagColor::Blue))
        .expect("groupe travail");
    let other = app.create_view_group("Autre", None).expect("groupe autre");
    let filter = tag_filter("facture");
    let first = app
        .create_view_in_group(
            ViewInput {
                filter: Some(filter.clone()),
                ..input("Premiere")
            },
            Some(work.id),
        )
        .expect("vue 1");
    let second = app
        .create_view_in_group(
            ViewInput {
                filter: Some(filter),
                ..input("Seconde")
            },
            Some(work.id),
        )
        .expect("vue 2");

    assert_eq!(
        app.views_in_group(work.id)
            .expect("vues")
            .into_iter()
            .map(|view| view.name)
            .collect::<Vec<_>>(),
        vec!["Seconde", "Premiere"]
    );

    app.assign_view(other.id, first.id).expect("assignation");
    assert!(
        app.views_in_group(work.id)
            .expect("travail")
            .iter()
            .all(|view| view.id != first.id)
    );
    app.move_view(other.id, work.id, first.id)
        .expect("deplacement");
    assert_eq!(app.views_in_group(work.id).expect("travail").len(), 2);

    let renamed = app
        .rename_view_group(other.id, "Autre 2")
        .expect("renommage");
    assert_eq!(renamed.name, "Autre 2");
    let collapsed = app.set_view_group_collapsed(other.id, true).expect("repli");
    assert!(collapsed.collapsed);

    assert!(app.delete_view_group(work.id).expect("suppression"));
    assert!(app.get_view(first.id).is_err());
    assert!(app.get_view(second.id).is_err());
    assert!(
        app.view_groups_with_views()
            .expect("groupes")
            .iter()
            .all(|(group, _)| group.id != work.id)
    );
}
