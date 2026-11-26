use super::*;

#[tokio::test]
async fn virtual_folder_tree_lists_and_serves_files() {
    let dav = setup().await;
    let router = dav.fixture.router();
    let file = dav
        .fixture
        .app
        .find_by_name("note.txt")
        .expect("recherche")
        .expect("fichier");
    let folder = dav
        .fixture
        .app
        .create_virtual_folder(None, "Photos")
        .expect("dossier");
    dav.fixture
        .app
        .move_files_to_virtual_folder(&[file.id], Some(folder.id))
        .expect("déplacement");

    let root = propfind_body(&router, "/dav/", &dav.token, "1").await;
    assert!(root.contains("virtual%20folders"), "{root}");

    let listing = propfind_body(&router, "/dav/virtual%20folders/", &dav.token, "1").await;
    assert!(listing.contains("Photos"), "{listing}");

    let inside = propfind_body(&router, "/dav/virtual%20folders/Photos/", &dav.token, "1").await;
    assert!(inside.contains(&dav.alias), "{inside}");

    let response = dav_get(
        &router,
        &format!("/dav/virtual%20folders/Photos/{}", dav.alias),
        &dav.token,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_text(response).await, "hello");
}

#[tokio::test]
async fn put_imports_files_into_existing_virtual_folder() {
    let dav = setup().await;
    let router = dav.fixture.router();
    assert_mkcol_created(&router, "/dav/virtual%20folders/ImportFiles/", &dav.token).await;

    for (uri, content) in [
        ("/dav/virtual%20folders/ImportFiles/a.txt", "a"),
        ("/dav/virtual%20folders/ImportFiles/b.txt", "b"),
    ] {
        assert_put_created(&router, uri, &dav.token, content).await;
    }

    let folder = child(&dav.fixture.app, None, "ImportFiles");
    let mut files = files_of(&dav.fixture.app, folder.id);
    files.sort();
    let mut expected = vec![dav.fixture.file_id("a.txt"), dav.fixture.file_id("b.txt")];
    expected.sort();
    assert_eq!(files, expected);

    let listing = propfind_body(
        &router,
        "/dav/virtual%20folders/ImportFiles/",
        &dav.token,
        "1",
    )
    .await;
    assert!(listing.contains("a.txt"), "{listing}");
    assert!(listing.contains("b.txt"), "{listing}");

    let response = dav_get(
        &router,
        "/dav/virtual%20folders/ImportFiles/a.txt",
        &dav.token,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_text(response).await, "a");
}

#[tokio::test]
async fn mkcol_creates_missing_virtual_folder_chain() {
    let dav = setup().await;
    let router = dav.fixture.router();
    assert_mkcol_created(&router, "/dav/virtual%20folders/Projet/2024/", &dav.token).await;

    let root = child(&dav.fixture.app, None, "Projet");
    let nested = child(&dav.fixture.app, Some(root.id), "2024");
    assert!(files_of(&dav.fixture.app, nested.id).is_empty());
}

#[tokio::test]
async fn put_creates_missing_virtual_folder_chain() {
    let dav = setup().await;
    let router = dav.fixture.router();
    let uri = "/dav/virtual%20folders/Projet/2024/fichier.txt";
    assert_put_created(&router, uri, &dav.token, "nested").await;

    let root = child(&dav.fixture.app, None, "Projet");
    let nested = child(&dav.fixture.app, Some(root.id), "2024");
    assert_eq!(
        files_of(&dav.fixture.app, nested.id),
        vec![dav.fixture.file_id("fichier.txt")]
    );

    let listing = propfind_body(
        &router,
        "/dav/virtual%20folders/Projet/2024/",
        &dav.token,
        "1",
    )
    .await;
    assert!(listing.contains("fichier.txt"), "{listing}");
}
