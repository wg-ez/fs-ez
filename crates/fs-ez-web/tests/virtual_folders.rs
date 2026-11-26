//! Dossiers virtuels dans l'interface : navigation, déplacement et tags.
//!
//! Test manuel Chrome du mode déplacement, avec `Hyper` contenant `Screenshot` et un dossier `Hello` :
//! 1. Ouvrir `/virtual-folders/4` (Hyper), clic droit sur `Screenshot`, choisir `Déplacer`.
//! 2. Double-cliquer `..`, double-cliquer `Hello`, puis clic droit dans le vide ou sur `..`.
//! 3. Vérifier la présence de `Déplacer ici` et `Annuler le déplacement` dans le menu.
//! 4. `Annuler le déplacement` : menu normal restauré, aucun déplacement, sélection vidée.
//! 5. Rejouer puis `Déplacer ici` : POST `/partials/virtual-folders/move-selection`, toast de succès.
//! 6. Vérifier que `Screenshot` a quitté `Hyper` et apparaît dans `Hello`, puis tester retour/avant navigateur.

mod common;

use common::*;

async fn folder_fixture() -> (Fixture, i64, i64) {
    let fixture = Fixture::new(FixtureOptions::default().create_files_dir()).await;
    let path = fixture.root().join("note.txt");
    std::fs::write(&path, b"hello").expect("fichier");
    let file = fixture
        .app
        .ensure_file(&path, fs_ez_app::EnsureOptions { force: false })
        .expect("fichier");
    let folder = create(&fixture, None, "Photos");
    fixture
        .app
        .move_files_to_virtual_folder(&[file.id], Some(folder.id))
        .expect("déplacement");
    (fixture, folder.id.0, file.id.0)
}

fn parent_of(
    fixture: &Fixture,
    id: fs_ez_core::VirtualFolderId,
) -> Option<fs_ez_core::VirtualFolderId> {
    fixture.app.get_virtual_folder(id).expect("dossier").parent
}

fn create(
    fixture: &Fixture,
    parent: Option<fs_ez_core::VirtualFolderId>,
    name: &str,
) -> fs_ez_core::VirtualFolder {
    fixture
        .app
        .create_virtual_folder(parent, name)
        .expect("dossier créé")
}

async fn post(router: &Router, cookie: &str, uri: &str, body: &str) -> Response<Body> {
    send(router, with_cookie(post_form(uri, body), cookie)).await
}

async fn post_hx(router: &Router, cookie: &str, uri: &str, body: &str) -> Response<Body> {
    send(router, with_cookie(with_hx(post_form(uri, body)), cookie)).await
}

#[tokio::test]
async fn virtual_folder_pages_list_folders_and_files() {
    let (fixture, folder_id, _) = folder_fixture().await;
    let router = fixture.router();

    let (status, body) = page(&router, &fixture.cookie, "/virtual-folders").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Dossiers virtuels") || body.contains("Virtual folders"));
    assert!(body.contains("Photos"), "{body}");
    assert!(body.contains("data-folder-row="), "{body}");
    assert!(body.contains("table__folder-icon"), "{body}");
    assert!(body.contains("table__name-link--folder"), "{body}");
    assert!(body.contains("data-folder-nav"), "{body}");
    assert!(body.contains("hx-select=\"#files-page\""), "{body}");
    assert!(!body.contains("data-folder-up"), "{body}");
    assert!(!body.contains("class=\"breadcrumb\""), "{body}");
    assert!(body.contains("data-folder-action=\"create\""), "{body}");
    assert!(!body.contains("class=\"virtual-folders\""), "{body}");

    let (status, body) = page(
        &router,
        &fixture.cookie,
        &format!("/virtual-folders/{folder_id}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("note.txt"), "{body}");
    assert!(body.contains("data-folder-drop"), "{body}");
    assert!(body.contains("data-folder-up"), "{body}");
    assert!(body.contains("href=\"/virtual-folders\""), "{body}");
    assert!(!body.contains("class=\"breadcrumb\""), "{body}");
    assert!(body.contains("data-folder-create"), "{body}");
}

#[tokio::test]
async fn create_move_and_delete_folder_from_the_web() {
    let (fixture, _, _) = folder_fixture().await;
    let router = fixture.router();

    let response = post(
        &router,
        &fixture.cookie,
        "/partials/virtual-folders",
        "name=Archives&parent=",
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let created = fixture
        .app
        .virtual_folder_children(None)
        .expect("racine")
        .into_iter()
        .find(|folder| folder.name == "Archives")
        .expect("dossier créé");

    let rename = format!("/partials/virtual-folders/{}/rename", created.id.0);
    let response = post(&router, &fixture.cookie, &rename, "name=Archive").await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        fixture
            .app
            .get_virtual_folder(created.id)
            .expect("dossier")
            .name,
        "Archive"
    );

    let delete = format!("/partials/virtual-folders/{}/delete", created.id.0);
    let response = post(&router, &fixture.cookie, &delete, "").await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert!(
        fixture
            .app
            .store
            .get_virtual_folder(created.id)
            .expect("lecture")
            .is_none()
    );
}

#[tokio::test]
async fn move_selection_into_folder_via_partial() {
    let (fixture, folder_id, _) = folder_fixture().await;
    let router = fixture.router();
    let loose_path = fixture.root().join("loose.txt");
    std::fs::write(&loose_path, b"loose").expect("fichier");
    let loose = fixture
        .app
        .ensure_file(&loose_path, fs_ez_app::EnsureOptions { force: false })
        .expect("fichier");

    let response = post(
        &router,
        &fixture.cookie,
        "/partials/files/move",
        &format!("ids={}&folder={folder_id}", loose.id.0),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        fixture.app.store.folder_of_file(loose.id).expect("dossier"),
        Some(fs_ez_core::VirtualFolderId(folder_id))
    );
}

#[tokio::test]
async fn move_selection_endpoint_moves_files_and_folders_atomically() {
    let (fixture, photos_id, file_id) = folder_fixture().await;
    let router = fixture.router();
    let archives = create(&fixture, None, "Archives");
    let child = create(
        &fixture,
        Some(fs_ez_core::VirtualFolderId(photos_id)),
        "Enfant",
    );

    let body = format!(
        "folder={}&file_ids={}&folder_ids={}",
        archives.id.0, file_id, child.id.0
    );
    let response = post_hx(
        &router,
        &fixture.cookie,
        "/partials/virtual-folders/move-selection",
        &body,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        header_string(&response, header::HeaderName::from_static("hx-trigger"))
            .contains("data-changed")
    );
    assert_eq!(
        fixture
            .app
            .store
            .folder_of_file(fs_ez_core::FileId(file_id))
            .expect("dossier"),
        Some(archives.id)
    );
    assert_eq!(parent_of(&fixture, child.id), Some(archives.id));
}

#[tokio::test]
async fn move_selection_refuses_a_cycle_without_partial_move() {
    let (fixture, _, _) = folder_fixture().await;
    let router = fixture.router();
    let parent = create(&fixture, None, "Parent");
    let child = create(&fixture, Some(parent.id), "Enfant");
    let other = create(&fixture, None, "Autre");

    let body = format!(
        "folder={}&folder_ids={}&folder_ids={}",
        child.id.0, other.id.0, parent.id.0
    );
    let response = post(
        &router,
        &fixture.cookie,
        "/partials/virtual-folders/move-selection",
        &body,
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        parent_of(&fixture, other.id),
        None,
        "aucun dossier ne bouge avant validation complète"
    );
    assert_eq!(parent_of(&fixture, parent.id), None);
}

#[tokio::test]
async fn delete_selection_removes_files_and_nested_folders_in_one_request() {
    let (fixture, photos_id, file_id) = folder_fixture().await;
    let router = fixture.router();
    let child = create(
        &fixture,
        Some(fs_ez_core::VirtualFolderId(photos_id)),
        "Enfant",
    );

    let body = format!(
        "file_ids={file_id}&folder_ids={photos_id}&folder_ids={}",
        child.id.0
    );
    let response = post_hx(
        &router,
        &fixture.cookie,
        "/partials/virtual-folders/delete-selection",
        &body,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        header_string(&response, header::HeaderName::from_static("hx-trigger"))
            .contains("data-changed")
    );
    let (_, body) = page(&router, &fixture.cookie, "/virtual-folders").await;
    assert!(!body.contains("Photos"), "{body}");
    assert!(
        fixture
            .app
            .store
            .get_virtual_folder(child.id)
            .expect("enfant")
            .is_none()
    );
    assert!(
        fixture
            .app
            .store
            .get_file(fs_ez_core::FileId(file_id))
            .expect("fichier")
            .is_none()
    );
}

#[tokio::test]
async fn delete_selection_ignores_a_folder_already_removed_with_its_parent() {
    let (fixture, _, _) = folder_fixture().await;
    let router = fixture.router();
    let parent = create(&fixture, None, "Parent");
    let child = create(&fixture, Some(parent.id), "Enfant");

    let body = format!("folder_ids={}&folder_ids={}", parent.id.0, child.id.0);
    let response = post_hx(
        &router,
        &fixture.cookie,
        "/partials/virtual-folders/delete-selection",
        &body,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        header_string(&response, header::HeaderName::from_static("hx-trigger"))
            .contains("data-changed")
    );
    assert!(
        fixture
            .app
            .store
            .get_virtual_folder(parent.id)
            .expect("dossier")
            .is_none()
    );
    assert!(
        fixture
            .app
            .store
            .get_virtual_folder(child.id)
            .expect("enfant")
            .is_none()
    );
}

#[tokio::test]
async fn tag_folder_applies_to_descendant_files() {
    let (fixture, folder_id, file_id) = folder_fixture().await;
    let router = fixture.router();

    let tags = format!("/partials/virtual-folders/{folder_id}/tags");
    let response = post_hx(&router, &fixture.cookie, &tags, "tags=archive").await;
    assert_eq!(response.status(), StatusCode::OK);
    let tags = fixture
        .app
        .store
        .tags_for_file(fs_ez_core::FileId(file_id))
        .expect("tags");
    assert!(tags.iter().any(|tag| tag.tag.name == "archive"));

    let (status, body) = page(&router, &fixture.cookie, "/virtual-folders").await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.contains("folder-tags-"), "{body}");
    assert!(!body.contains("data-folder-id=\""), "{body}");
}

#[tokio::test]
async fn folder_rows_fragment_returns_a_bounded_window() {
    let (fixture, folder_id, _) = folder_fixture().await;
    let router = fixture.router();
    for name in ["Alpha", "Beta", "Gamma"] {
        create(&fixture, Some(fs_ez_core::VirtualFolderId(folder_id)), name);
    }

    let (status, body) = page(
        &router,
        &fixture.cookie,
        &format!("/virtual-folders/{folder_id}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("data-folder-total=\"3\""), "{body}");
    assert!(
        body.contains(&format!(
            "data-folder-rows-url=\"/partials/virtual-folders/rows?parent={folder_id}\""
        )),
        "{body}"
    );

    let response = send(
        &router,
        with_cookie(
            get(&format!(
                "/partials/virtual-folders/rows?parent={folder_id}&offset=1&limit=1"
            )),
            &fixture.cookie,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        header_string(&response, header::HeaderName::from_static("vary")).contains("HX-Request")
    );
    let body = body_text(response).await;
    assert_eq!(body.matches("data-folder-row=").count(), 1, "{body}");
    assert!(body.contains("Beta"), "{body}");
    assert!(!body.contains("Alpha"), "{body}");
    assert!(!body.contains("data-folder-up"), "{body}");
}

#[tokio::test]
async fn bulk_tag_removal_accepts_selected_folders() {
    let (fixture, folder_id, file_id) = folder_fixture().await;
    let router = fixture.router();

    post_hx(
        &router,
        &fixture.cookie,
        &format!("/partials/virtual-folders/{folder_id}/tags"),
        "tags=archive",
    )
    .await;

    let response = post_hx(
        &router,
        &fixture.cookie,
        "/partials/files/tags",
        &format!("folder_ids={folder_id}&tags=archive&action=remove"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let tags = fixture
        .app
        .store
        .tags_for_file(fs_ez_core::FileId(file_id))
        .expect("tags");
    assert!(tags.is_empty(), "{tags:?}");
}
