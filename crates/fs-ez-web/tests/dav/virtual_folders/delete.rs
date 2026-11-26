use super::*;

#[tokio::test]
async fn mkcol_and_delete_virtual_folders_deletes_the_file_tree() {
    let dav = setup().await;
    let router = dav.fixture.router();
    for directory in [
        "/dav/virtual%20folders/TestDAV/",
        "/dav/virtual%20folders/TestDAV/Sous/",
    ] {
        assert_mkcol_created(&router, directory, &dav.token).await;
    }

    let root = child(&dav.fixture.app, None, "TestDAV");
    let nested = child(&dav.fixture.app, Some(root.id), "Sous");
    assert!(files_of(&dav.fixture.app, nested.id).is_empty());

    let file = dav
        .fixture
        .app
        .find_by_name("note.txt")
        .expect("recherche")
        .expect("fichier");
    dav.fixture
        .app
        .move_files_to_virtual_folder(&[file.id], Some(nested.id))
        .expect("déplacement");
    assert!(dav.fixture.root().join("note.txt").exists());

    let response = send(
        &router,
        dav_request(
            Method::DELETE,
            "/dav/virtual%20folders/TestDAV/",
            &dav.token,
            Body::empty(),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(
        dav.fixture
            .app
            .store
            .get_virtual_folder(root.id)
            .expect("dossier")
            .is_none()
    );
    assert!(
        dav.fixture
            .app
            .store
            .get_virtual_folder(nested.id)
            .expect("enfant")
            .is_none()
    );
    assert!(
        dav.fixture
            .app
            .store
            .get_file(file.id)
            .expect("fichier")
            .is_none()
    );
    assert!(!dav.fixture.root().join("note.txt").exists());
}
