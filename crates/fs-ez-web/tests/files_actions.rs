//! Menu contextuel des fichiers : renommage et suppression.

mod common;

use common::*;
use fs_ez_app::EnsureOptions;
use fs_ez_core::FileId;

async fn fixture() -> Fixture {
    common::default_fixture().await
}

fn add_file(fixture: &Fixture, name: &str) -> FileId {
    let path = fixture.root().join(name);
    std::fs::write(&path, name.as_bytes()).expect("fichier");
    fixture
        .app
        .ensure_file(&path, EnsureOptions { force: false })
        .expect("fichier")
        .id
}

#[tokio::test]
async fn rename_moves_the_file_and_triggers_a_refresh() {
    let fixture = fixture().await;
    let id = add_file(&fixture, "avant.txt");

    let response = fixture
        .post_hx(&format!("/partials/files/{id}/rename"), "name=apres.txt")
        .await;

    assert_status(&response, StatusCode::OK);
    assert_eq!(
        header_string(&response, header::HeaderName::from_static("hx-trigger")),
        "data-changed"
    );
    let html = body_text(response).await;
    assert!(html.contains("File renamed"), "{html}");
    assert!(!fixture.root().join("avant.txt").exists());
    assert!(fixture.root().join("apres.txt").exists());
    assert_eq!(
        fixture.app.file_with_tags(id).expect("fichier").file.name,
        "apres.txt"
    );
}

#[tokio::test]
async fn rename_conflict_keeps_the_original_file() {
    let fixture = fixture().await;
    let id = add_file(&fixture, "avant.txt");
    add_file(&fixture, "occupe.txt");

    let response = fixture
        .post_hx(&format!("/partials/files/{id}/rename"), "name=occupe.txt")
        .await;

    assert_status(&response, StatusCode::OK);
    let html = body_text(response).await;
    assert!(html.contains("toast--error"), "{html}");
    assert!(fixture.root().join("avant.txt").exists());
    assert_eq!(
        fixture.app.file_with_tags(id).expect("fichier").file.name,
        "avant.txt"
    );
}

#[tokio::test]
async fn delete_removes_files_from_disk_and_index() {
    let fixture = fixture().await;
    let first = add_file(&fixture, "un.txt");
    let second = add_file(&fixture, "deux.txt");
    let body = format!("ids={first}&ids={second}");

    let response = fixture.post_hx("/partials/files/delete", &body).await;

    assert_status(&response, StatusCode::OK);
    assert_eq!(
        header_string(&response, header::HeaderName::from_static("hx-trigger")),
        "data-changed"
    );
    let html = body_text(response).await;
    assert!(html.contains("2 files deleted"), "{html}");
    assert!(!fixture.root().join("un.txt").exists());
    assert!(!fixture.root().join("deux.txt").exists());
    assert!(
        fixture
            .app
            .store
            .get_file(first)
            .expect("lecture")
            .is_none()
    );
    assert!(
        fixture
            .app
            .store
            .get_file(second)
            .expect("lecture")
            .is_none()
    );
}

#[tokio::test]
async fn delete_scope_all_removes_every_matching_file() {
    let fixture = fixture().await;
    let first = add_file(&fixture, "un.txt");
    let second = add_file(&fixture, "deux.txt");
    let body = format!(
        "ids={first}&scope=all&scope_url={}",
        encode("/?sort=id:asc")
    );

    let response = fixture.post_hx("/partials/files/delete", &body).await;

    assert_status(&response, StatusCode::OK);
    let html = body_text(response).await;
    assert!(html.contains("2 files deleted"), "{html}");
    assert!(!fixture.root().join("un.txt").exists());
    assert!(!fixture.root().join("deux.txt").exists());
    assert!(
        fixture
            .app
            .store
            .get_file(second)
            .expect("lecture")
            .is_none()
    );
}

#[tokio::test]
async fn delete_scope_all_skips_excluded_ids() {
    let fixture = fixture().await;
    let first = add_file(&fixture, "un.txt");
    let kept = add_file(&fixture, "garde.txt");
    let third = add_file(&fixture, "trois.txt");
    let body = format!(
        "ids={first}&scope=all&exclude_ids={kept}&scope_url={}",
        encode("/?sort=id:asc")
    );

    let response = fixture.post_hx("/partials/files/delete", &body).await;

    assert_status(&response, StatusCode::OK);
    let html = body_text(response).await;
    assert!(html.contains("2 files deleted"), "{html}");
    assert!(!fixture.root().join("un.txt").exists());
    assert!(!fixture.root().join("trois.txt").exists());
    assert!(fixture.root().join("garde.txt").exists());
    assert!(fixture.app.store.get_file(kept).expect("lecture").is_some());
    assert!(
        fixture
            .app
            .store
            .get_file(first)
            .expect("lecture")
            .is_none()
    );
    assert!(
        fixture
            .app
            .store
            .get_file(third)
            .expect("lecture")
            .is_none()
    );
}

#[tokio::test]
async fn delete_without_selection_returns_a_toast() {
    let fixture = fixture().await;

    let response = fixture.post_hx("/partials/files/delete", "").await;

    assert_status(&response, StatusCode::OK);
    let html = body_text(response).await;
    assert!(html.contains("toast--error"), "{html}");
    assert!(html.contains("No file selected"), "{html}");
}

#[tokio::test]
async fn cross_site_delete_is_forbidden() {
    let fixture = fixture().await;
    let id = add_file(&fixture, "garde.txt");

    let response = fixture
        .send(with_cookie(
            with_site(
                post_form("/partials/files/delete", &format!("ids={id}")),
                "cross-site",
            ),
            &fixture.cookie,
        ))
        .await;

    assert_status(&response, StatusCode::FORBIDDEN);
    assert!(fixture.root().join("garde.txt").exists());
    assert!(fixture.app.store.get_file(id).expect("lecture").is_some());
}
