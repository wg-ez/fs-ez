use fs_ez_app::{AppError, EnsureOptions, ViewInput};
use fs_ez_core::{FilterExpr, TagColor, virtual_file_name};

use super::support::*;
use crate::common::*;

fn tag(name: &str) -> FilterExpr {
    FilterExpr::Tag {
        tag: name.to_string(),
        negated: false,
    }
}

async fn create_view(dav: &Dav, name: &str, filter: FilterExpr) {
    dav.fixture
        .app
        .create_view(ViewInput {
            name: name.to_string(),
            filter: Some(filter),
            icon: None,
            color: None,
            position: None,
            sort: Default::default(),
        })
        .expect("vue");
}

#[tokio::test]
async fn put_into_saved_view_imports_with_filter_tags() {
    let dav = setup().await;
    let router = dav.fixture.router();
    dav.fixture.create_tag("archive", TagColor::Amber);
    create_view(
        &dav,
        "combo",
        FilterExpr::And {
            items: vec![tag(TAG), tag("archive")],
        },
    )
    .await;

    let uri = format!("/dav/views/{VIEW_GROUP}/combo/photo.jpg");
    let response = send(
        &router,
        dav_request(Method::PUT, &uri, &dav.token, Body::from("photo")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let id = dav.fixture.file_id("photo.jpg");
    let mut tags = dav.fixture.tag_names(id);
    tags.sort();
    assert_eq!(tags, ["archive".to_string(), TAG.to_string()]);

    let body = propfind_body(
        &router,
        &format!("/dav/views/{VIEW_GROUP}/combo/"),
        &dav.token,
        "1",
    )
    .await;
    assert!(body.contains("photo.jpg"), "{body}");
}

#[tokio::test]
async fn put_into_view_with_too_many_filter_tags_is_refused() {
    let dav = setup().await;
    let router = dav.fixture.router();
    let tags: Vec<String> = (0..=fs_ez_app::tagspec::MAX_TAGS_PER_REQUEST)
        .map(|index| format!("bulk{index:03}"))
        .collect();
    let before = dav.fixture.app.list_tags().expect("tags").len();
    create_view(
        &dav,
        "too-many",
        FilterExpr::TagSet {
            tags,
            negated: false,
        },
    )
    .await;

    let uri = format!("/dav/views/{VIEW_GROUP}/too-many/too-many-note.txt");
    let response = send(
        &router,
        dav_request(Method::PUT, &uri, &dav.token, Body::from("too many")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(dav.fixture.app.list_tags().expect("tags").len(), before);
}

#[tokio::test]
async fn apply_tags_enforces_tag_cap() {
    let dav = setup().await;
    let id = dav.fixture.file_id("note.txt");
    let too_many: Vec<String> = (0..=fs_ez_app::tagspec::MAX_TAGS_PER_REQUEST)
        .map(|index| format!("cap{index:03}"))
        .collect();
    match dav.fixture.app.apply_tags(id, &too_many, true) {
        Err(AppError::Invalid(message)) => assert_eq!(message, "too many tags (max 256)"),
        other => panic!("257 tags acceptés: {other:?}"),
    }
    let at_cap: Vec<String> = (0..fs_ez_app::tagspec::MAX_TAGS_PER_REQUEST)
        .map(|index| format!("cap{index:03}"))
        .collect();
    dav.fixture
        .app
        .apply_tags(id, &at_cap, true)
        .expect("256 tags");
    assert_eq!(
        dav.fixture
            .app
            .file_with_tags(id)
            .expect("fichier")
            .tags
            .len(),
        1 + fs_ez_app::tagspec::MAX_TAGS_PER_REQUEST
    );
}

#[tokio::test]
async fn put_into_view_without_required_tags_is_refused() {
    let dav = setup().await;
    let router = dav.fixture.router();
    create_view(
        &dav,
        "without-work",
        FilterExpr::Not {
            item: Box::new(tag(TAG)),
        },
    )
    .await;

    let uri = format!("/dav/views/{VIEW_GROUP}/without-work/absent.jpg");
    let response = send(
        &router,
        dav_request(Method::PUT, &uri, &dav.token, Body::from("absent")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(!dav.fixture.root().join("absent.jpg").exists());
}

async fn smart_view_round_trip(view: &str, tagged: bool) {
    let dav = setup().await;
    let router = dav.fixture.router();
    let (name, path) = if tagged {
        ("note.txt", dav.fixture.root().join("note.txt"))
    } else {
        let path = dav.fixture.root().join("loose.txt");
        std::fs::write(&path, b"loose").expect("écriture");
        dav.fixture
            .app
            .ensure_file(&path, EnsureOptions { force: false })
            .expect("fichier");
        ("loose.txt", path)
    };
    let alias = virtual_file_name(
        &dav.fixture
            .app
            .store
            .find_file_by_path(&path)
            .expect("lecture")
            .expect("fichier"),
    );

    let uri = format!("/dav/{view}/{alias}");
    let response = send(
        &router,
        dav_request(Method::PUT, &uri, &dav.token, Body::from("updated")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT, "{view}");
    assert_eq!(std::fs::read(&path).expect("contenu"), b"updated");

    let destination = format!("/dav/{view}/renamed-{name}");
    let response = move_folder(&router, &uri, &destination, &dav.token).await;
    assert_eq!(response.status(), StatusCode::CREATED, "{view}");
    let renamed = dav.fixture.root().join(format!("renamed-{name}"));
    assert_eq!(std::fs::read(&renamed).expect("contenu"), b"updated");

    let response = send(
        &router,
        dav_request(Method::DELETE, &destination, &dav.token, Body::empty()),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT, "{view}");
    assert!(!renamed.exists());
}

#[tokio::test]
async fn smart_views_allow_put_move_and_delete() {
    smart_view_round_trip("untagged", false).await;
    smart_view_round_trip("recently%20tagged", true).await;
    smart_view_round_trip("recent%20files", true).await;
}
