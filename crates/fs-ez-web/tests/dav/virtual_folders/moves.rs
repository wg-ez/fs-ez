use super::*;

#[tokio::test]
async fn move_renames_virtual_folder_and_keeps_files() {
    let dav = setup().await;
    let router = dav.fixture.router();
    assert_mkcol_created(&router, "/dav/virtual%20folders/ImportFiles/", &dav.token).await;
    for (uri, content) in [
        ("/dav/virtual%20folders/ImportFiles/a.txt", "a"),
        ("/dav/virtual%20folders/ImportFiles/b.txt", "b"),
    ] {
        assert_put_created(&router, uri, &dav.token, content).await;
    }
    let old = child(&dav.fixture.app, None, "ImportFiles");
    let mut expected = files_of(&dav.fixture.app, old.id);

    let response = move_folder(
        &router,
        "/dav/virtual%20folders/ImportFiles/",
        "/dav/virtual%20folders/RenamedFiles/",
        &dav.token,
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    assert_no_child(
        &dav.fixture.app,
        None,
        &["ImportFiles"],
        "l'ancien nom doit disparaître",
    );
    let renamed = child(&dav.fixture.app, None, "RenamedFiles");
    expected.sort();
    let mut files = files_of(&dav.fixture.app, renamed.id);
    files.sort();
    assert_eq!(files, expected);

    let response = send(
        &router,
        propfind("/dav/virtual%20folders/ImportFiles/", &dav.token, "1"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let listing = propfind_body(
        &router,
        "/dav/virtual%20folders/RenamedFiles/",
        &dav.token,
        "1",
    )
    .await;
    assert!(listing.contains("a.txt"), "{listing}");
    assert!(listing.contains("b.txt"), "{listing}");
}

#[tokio::test]
async fn move_virtual_folder_to_another_parent_keeps_files() {
    let dav = setup().await;
    let router = dav.fixture.router();
    for directory in [
        "/dav/virtual%20folders/Source/",
        "/dav/virtual%20folders/Source/Enfant/",
        "/dav/virtual%20folders/Cible/",
    ] {
        assert_mkcol_created(&router, directory, &dav.token).await;
    }
    let uri = "/dav/virtual%20folders/Source/Enfant/fichier.txt";
    assert_put_created(&router, uri, &dav.token, "enfant").await;

    let response = move_folder(
        &router,
        "/dav/virtual%20folders/Source/Enfant/",
        "/dav/virtual%20folders/Cible/Enfant/",
        &dav.token,
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let source = child(&dav.fixture.app, None, "Source");
    assert!(
        dav.fixture
            .app
            .virtual_folder_children(Some(source.id))
            .expect("source")
            .is_empty()
    );
    let target = child(&dav.fixture.app, None, "Cible");
    let moved = child(&dav.fixture.app, Some(target.id), "Enfant");
    assert_eq!(
        files_of(&dav.fixture.app, moved.id),
        vec![dav.fixture.file_id("fichier.txt")]
    );
}

#[tokio::test]
async fn move_virtual_folder_refuses_existing_destination() {
    let dav = setup().await;
    let router = dav.fixture.router();
    for directory in [
        "/dav/virtual%20folders/Premier/",
        "/dav/virtual%20folders/Second/",
    ] {
        assert_mkcol_created(&router, directory, &dav.token).await;
    }

    let response = move_folder(
        &router,
        "/dav/virtual%20folders/Premier/",
        "/dav/virtual%20folders/Second/",
        &dav.token,
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);

    assert!(child(&dav.fixture.app, None, "Premier").name == "Premier");
    assert!(child(&dav.fixture.app, None, "Second").name == "Second");
}

#[tokio::test]
async fn move_alias_onto_existing_virtual_folder_keeps_destination_files() {
    let dav = setup().await;
    let router = dav.fixture.router();
    assert_mkcol_created(&router, "/dav/virtual%20folders/Keep/", &dav.token).await;
    assert_put_created(
        &router,
        "/dav/virtual%20folders/Keep/keep.txt",
        &dav.token,
        "keep-content",
    )
    .await;

    let keep = child(&dav.fixture.app, None, "Keep");
    let keep_id = dav.fixture.file_id("keep.txt");
    let mut expected = files_of(&dav.fixture.app, keep.id);
    assert_eq!(expected, vec![keep_id]);
    let source = tag_file_uri(TAG, &dav.alias);

    let response = move_folder(&router, &source, "/dav/virtual%20folders/Keep/", &dav.token).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);

    let record = dav
        .fixture
        .app
        .store
        .get_file(keep_id)
        .expect("lecture")
        .expect("fichier");
    assert_eq!(
        std::fs::read(&record.path).expect("contenu"),
        b"keep-content"
    );
    expected.sort();
    let mut files = files_of(&dav.fixture.app, keep.id);
    files.sort();
    assert_eq!(files, expected);

    let response = dav_get(&router, "/dav/virtual%20folders/Keep/keep.txt", &dav.token).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_text(response).await, "keep-content");

    let response = dav_get(&router, &source, &dav.token).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_text(response).await, "hello");
}
