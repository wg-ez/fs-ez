use fs_ez_core::TagColor;

use super::*;

#[tokio::test]
async fn nested_inbox_upload_creates_virtual_folder_tree() {
    let (fixture, token) = inbox_fixture().await;
    let router = fixture.router();
    for directory in ["/dav/inbox/ImportTest", "/dav/inbox/ImportTest/Sous"] {
        assert_mkcol_created(&router, directory, &token).await;
    }

    let uri = "/dav/inbox/ImportTest/Sous/fichier.txt";
    assert_put_created(&router, uri, &token, "contenu").await;

    let file = fixture.file_id("fichier.txt");
    let date = inbox_date(&fixture.app);
    let root = child(&fixture.app, Some(date.id), "ImportTest");
    let nested = child(&fixture.app, Some(root.id), "Sous");
    assert_eq!(files_of(&fixture.app, nested.id), vec![file]);

    let body = propfind_body(&router, "/dav/inbox/ImportTest/", &token, "1").await;
    assert!(
        !body.contains("Sous"),
        "dossier importé absent du listing inbox : {body}"
    );
    let body = propfind_body(&router, "/dav/inbox/ImportTest/Sous/", &token, "1").await;
    assert!(
        !body.contains("fichier.txt"),
        "fichier importé absent du listing inbox : {body}"
    );
}

#[tokio::test]
async fn mkcol_creates_empty_virtual_folders() {
    let (fixture, token) = inbox_fixture().await;
    let router = fixture.router();
    for directory in ["/dav/inbox/Projet", "/dav/inbox/Projet/2024"] {
        assert_mkcol_created(&router, directory, &token).await;
    }

    let date = inbox_date(&fixture.app);
    let root = child(&fixture.app, None, "inbox");
    assert_eq!(date.parent, Some(root.id));
    let project = child(&fixture.app, Some(date.id), "Projet");
    let empty = child(&fixture.app, Some(project.id), "2024");
    assert!(files_of(&fixture.app, empty.id).is_empty());
}

#[tokio::test]
async fn nested_tag_upload_applies_tags_and_creates_virtual_folder() {
    let dav = setup().await;
    let router = dav.fixture.router();
    let uri = format!("/dav/tags/{TAG}/Projet/Sous/fichier.txt");

    assert_put_created(&router, &uri, &dav.token, "nested").await;

    let file = dav.fixture.file_id("fichier.txt");
    assert_eq!(dav.fixture.tag_names(file), vec![TAG.to_string()]);
    let date = inbox_date(&dav.fixture.app);
    let project = child(&dav.fixture.app, Some(date.id), "Projet");
    let nested = child(&dav.fixture.app, Some(project.id), "Sous");
    assert_eq!(files_of(&dav.fixture.app, nested.id), vec![file]);
    assert_no_child(
        &dav.fixture.app,
        None,
        &["Projet", "Sous"],
        "l'import tag passe sous inbox/<date>, sans dossier parasite à la racine",
    );
}

#[tokio::test]
async fn direct_tag_upload_goes_into_dated_inbox() {
    let dav = setup().await;
    let router = dav.fixture.router();
    let uri = format!("/dav/tags/{TAG}/tag-import-test.txt");

    assert_put_created(&router, &uri, &dav.token, "direct").await;

    let file = dav.fixture.file_id("tag-import-test.txt");
    assert_eq!(dav.fixture.tag_names(file), vec![TAG.to_string()]);
    let date = inbox_date(&dav.fixture.app);
    assert_eq!(files_of(&dav.fixture.app, date.id), vec![file]);
    assert_no_child(
        &dav.fixture.app,
        Some(date.id),
        &["All Files", TAG],
        "les marqueurs et noms de tags ne deviennent pas des dossiers",
    );
}

#[tokio::test]
async fn mkcol_under_tags_creates_dated_inbox_folders() {
    let dav = setup().await;
    let router = dav.fixture.router();
    for directory in [
        format!("/dav/tags/{TAG}/TagImport"),
        format!("/dav/tags/{TAG}/TagImport/Sous"),
    ] {
        assert_mkcol_created(&router, &directory, &dav.token).await;
    }

    let date = inbox_date(&dav.fixture.app);
    let root = child(&dav.fixture.app, Some(date.id), "TagImport");
    let nested = child(&dav.fixture.app, Some(root.id), "Sous");
    assert!(files_of(&dav.fixture.app, nested.id).is_empty());
}

#[tokio::test]
async fn all_files_marker_is_not_a_virtual_folder() {
    let dav = setup().await;
    let router = dav.fixture.router();
    let uri = format!("/dav/tags/{TAG}/{FILES}/Album/photo.txt");

    assert_put_created(&router, &uri, &dav.token, "photo").await;

    let file = dav.fixture.file_id("photo.txt");
    assert_eq!(dav.fixture.tag_names(file), vec![TAG.to_string()]);
    let date = inbox_date(&dav.fixture.app);
    let album = child(&dav.fixture.app, Some(date.id), "Album");
    assert_eq!(files_of(&dav.fixture.app, album.id), vec![file]);
    assert_no_child(
        &dav.fixture.app,
        Some(date.id),
        &["All Files"],
        "le marqueur All Files ne doit pas devenir un dossier",
    );
}

#[tokio::test]
async fn all_empty_marker_is_not_a_virtual_folder() {
    let dav = setup().await;
    let extra = dav.fixture.create_tag("Empty", TagColor::Red);
    let router = dav.fixture.router();
    let uri = format!("/dav/tags/All%20Empty/{}/All%20Files/first.txt", extra.name);

    assert_put_created(&router, &uri, &dav.token, "first").await;

    let file = dav.fixture.file_id("first.txt");
    assert_eq!(dav.fixture.tag_names(file), vec![extra.name.clone()]);
    let date = inbox_date(&dav.fixture.app);
    assert_eq!(files_of(&dav.fixture.app, date.id), vec![file]);
    assert_no_child(
        &dav.fixture.app,
        Some(date.id),
        &["All Empty", extra.name.as_str()],
        "le marqueur All Empty et le nom de tag ne deviennent pas des dossiers",
    );
}

#[tokio::test]
async fn multi_tag_path_does_not_create_tag_folders() {
    let dav = setup().await;
    let extra = dav.fixture.create_tag("Projet", TagColor::Green);
    dav.fixture
        .app
        .tag_path(
            &dav.fixture.root().join("note.txt"),
            std::slice::from_ref(&extra.name),
            false,
        )
        .expect("second tag");
    let router = dav.fixture.router();
    let uri = format!("/dav/tags/{TAG}/{}/Album/fichier.txt", extra.name);

    assert_put_created(&router, &uri, &dav.token, "multi").await;

    let file = dav.fixture.file_id("fichier.txt");
    let mut tags = dav.fixture.tag_names(file);
    tags.sort();
    assert_eq!(tags, [extra.name.clone(), TAG.to_string()]);
    let date = inbox_date(&dav.fixture.app);
    let album = child(&dav.fixture.app, Some(date.id), "Album");
    assert_eq!(files_of(&dav.fixture.app, album.id), vec![file]);
    assert_no_child(
        &dav.fixture.app,
        Some(date.id),
        &[extra.name.as_str(), TAG],
        "les noms de tags sélectionnés ne deviennent pas des dossiers",
    );
}
