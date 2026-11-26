use super::*;

#[tokio::test]
async fn cli_and_ui_share_the_same_database() {
    let fixture = Fixture::new(FixtureOptions::default().persistent_db().create_files_dir()).await;
    std::fs::create_dir_all(fixture.inbox()).expect("inbox");
    std::fs::write(fixture.inbox().join("partage.txt"), b"partage").expect("fichier");
    let response = fixture.post("/api/inbox/scan", "").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!fixture.inbox().join("partage.txt").exists());

    let root = fixture.root().to_path_buf();
    let db_path = fixture.db_path.clone();
    let listed = tokio::task::spawn_blocking(move || cli_json(&root, &db_path, &["files"]))
        .await
        .expect("tâche CLI");
    let items = listed["items"].as_array().expect("items");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["file"]["name"], "partage.txt");
}
