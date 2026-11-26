use std::ffi::OsStr;
use std::sync::Arc;

use fs_ez_app::{App, EnsureOptions, token};
use fs_ez_core::{Tag, virtual_file_name};
use fs_ez_fingerprint::Blake3Fingerprinter;
use fs_ez_fs::{Root, SystemFs};
use fs_ez_sqlite::{SqliteStore, db};
use fs_ez_store::Store;
use tempfile::TempDir;

use super::*;

fn fixture() -> (TempDir, App, Tag, String) {
    let temp = TempDir::new().expect("tempdir");
    let path = temp.path().join("note.txt");
    std::fs::write(&path, b"hello").expect("fichier");
    let store = Arc::new(SqliteStore::from_connection(
        db::open_in_memory().expect("base"),
    ));
    let app = App::new(
        store as Arc<dyn Store>,
        Arc::new(SystemFs),
        Arc::new(Blake3Fingerprinter::default()),
        Root::discover(Some(temp.path())).expect("racine"),
    );
    let file = app
        .ensure_file(&path, EnsureOptions { force: false })
        .expect("fichier");
    app.tag_path(&path, &["Txt Test".to_string()], false)
        .expect("tag");
    let tag = app
        .list_tags()
        .expect("tags")
        .into_iter()
        .find(|counted| counted.tag.name == "Txt Test")
        .expect("tag")
        .tag;
    (temp, app, tag, virtual_file_name(&file))
}

fn components(alias: &str) -> Vec<String> {
    vec![
        "tags".to_string(),
        "Txt Test".to_string(),
        FILES_DIR.to_string(),
        alias.to_string(),
    ]
}

fn file_id(node: &Node) -> fs_ez_core::FileId {
    match node {
        Node::File { record, .. } => record.id,
        other => panic!("attendu un fichier, obtenu {other:?}"),
    }
}

#[test]
fn resolve_with_cache_matches_resolve_without_cache() {
    let (_temp, app, _tag, alias) = fixture();
    let components = components(&alias);
    let plain = resolve_with(&app, &components, None).expect("sans cache");
    let cache = SelectionCache::default();
    let first = resolve_with(&app, &components, Some(&cache)).expect("premier");
    let second = resolve_with(&app, &components, Some(&cache)).expect("second");

    assert_eq!(file_id(&plain), file_id(&first));
    assert_eq!(file_id(&first), file_id(&second));
}

#[test]
fn selection_cache_is_bounded_and_cleared_on_invalidation() {
    let (_temp, app, tag, _alias) = fixture();
    let cache = SelectionCache::default();
    assert!(
        cache
            .complete(&app, std::slice::from_ref(&tag))
            .expect("complete")
    );
    assert!(
        cache
            .exists(&app, std::slice::from_ref(&tag))
            .expect("exists")
    );

    let file = app
        .store
        .all_files()
        .expect("fichiers")
        .into_iter()
        .next()
        .expect("fichier");
    app.delete_files(&[file.id]).expect("suppression");

    // TTL court : la lecture immédiate reste servie par le cache.
    assert!(
        cache
            .exists(&app, std::slice::from_ref(&tag))
            .expect("stale")
    );

    cache.clear();
    assert!(
        !cache
            .exists(&app, std::slice::from_ref(&tag))
            .expect("frais")
    );
    assert!(
        cache
            .complete(&app, std::slice::from_ref(&tag))
            .expect("vide")
    );
}

#[test]
fn tag_children_cache_is_cleared_with_the_selection_cache() {
    let (_temp, app, _tag, _alias) = fixture();
    let cache = SelectionCache::default();
    assert!(cache.tag_children(&app).expect("tags").len() == 1);
    let file = app
        .store
        .all_files()
        .expect("fichiers")
        .into_iter()
        .next()
        .expect("fichier");
    app.delete_files(&[file.id]).expect("suppression");

    assert!(cache.tag_children(&app).expect("stale").len() == 1);
    cache.clear();
    assert!(cache.tag_children(&app).expect("frais").is_empty());
}

#[test]
fn destination_resolution_skips_alias_scan_and_finds_member() {
    let (_temp, app, _tag, alias) = fixture();
    let cache = SelectionCache::default();
    let node = resolve_with(&app, &components(&alias), Some(&cache)).expect("source");
    assert!(matches!(node, Node::File { .. }));
    assert!(structure_is_file(&components(&alias)));
    let node = resolve_with(&app, &components("absent.txt"), Some(&cache)).expect("destination");
    assert!(matches!(node, Node::Unknown { .. }));
}

#[test]
fn homonym_outside_the_selection_does_not_mask_the_member() {
    let temp = TempDir::new().expect("tempdir");
    let first = temp.path().join("a");
    let second = temp.path().join("b");
    std::fs::create_dir_all(&first).expect("a");
    std::fs::create_dir_all(&second).expect("b");
    let outside = first.join("note.txt");
    let member = second.join("note.txt");
    std::fs::write(&outside, b"outside").expect("hors selection");
    std::fs::write(&member, b"member").expect("membre");

    let store = Arc::new(SqliteStore::from_connection(
        db::open_in_memory().expect("base"),
    ));
    let app = App::new(
        store as Arc<dyn Store>,
        Arc::new(SystemFs),
        Arc::new(Blake3Fingerprinter::default()),
        Root::discover(Some(temp.path())).expect("racine"),
    );
    let outside = app
        .ensure_file(&outside, EnsureOptions { force: false })
        .expect("hors selection");
    let member_record = app
        .tag_path(&member, &["Txt Test".to_string()], false)
        .expect("membre");
    assert!(outside.id.0 < member_record.file.id.0);

    let node = resolve_with(&app, &components("note.txt"), None).expect("membre");
    assert!(matches!(node, Node::File { record, .. } if record.id == member_record.file.id));
}

#[test]
fn rename_keeps_selection_cache_coherent() {
    let (_temp, app, tag, alias) = fixture();
    let cache = SelectionCache::default();
    let node = resolve_with(&app, &components(&alias), Some(&cache)).expect("avant");
    let Node::File { record, .. } = node else {
        panic!("attendu un fichier");
    };
    assert!(
        cache
            .exists(&app, std::slice::from_ref(&tag))
            .expect("exists")
    );
    assert!(
        cache
            .complete(&app, std::slice::from_ref(&tag))
            .expect("complete")
    );

    app.rename_resolved_file(record, "renamed.txt")
        .expect("renommage");
    assert!(matches!(
        resolve_with(&app, &components(&alias), Some(&cache)).expect("ancien"),
        Node::Unknown { .. }
    ));
    assert!(matches!(
        resolve_with(&app, &components("renamed.txt"), Some(&cache)).expect("nouveau"),
        Node::File { .. }
    ));
    // Un renommage ne change aucun tag : le cache de sélection reste valide.
    assert!(
        cache
            .exists(&app, std::slice::from_ref(&tag))
            .expect("exists")
    );
    assert!(
        cache
            .complete(&app, std::slice::from_ref(&tag))
            .expect("complete")
    );
}

#[test]
fn upload_folder_strips_tags_markers_and_entry_name() {
    let (_temp, app, tag, _alias) = fixture();
    let date = fs_ez_core::datetime::now_local().date().to_string();
    let inbox = Collection {
        kind: CollectionKind::Inbox,
        components: vec![
            "inbox".to_string(),
            "Projet".to_string(),
            "2024".to_string(),
            "fichier.txt".to_string(),
        ],
    };
    assert_eq!(
        inbox.upload_folder(&app),
        ["inbox", &date, "Projet", "2024"]
    );
    assert_eq!(
        inbox.collection_folder(&app),
        ["inbox", &date, "Projet", "2024", "fichier.txt"]
    );
    assert_eq!(inbox.upload_base_len(), 1);

    let inbox_root = Collection {
        kind: CollectionKind::Inbox,
        components: vec!["inbox".to_string()],
    };
    assert_eq!(inbox_root.upload_folder(&app), ["inbox", &date]);
    assert_eq!(inbox_root.collection_folder(&app), ["inbox", &date]);
    assert_eq!(inbox_root.upload_base_len(), 1);

    let tags = Collection {
        kind: CollectionKind::TagFiles {
            tags: vec![tag.clone(), tag.clone()],
        },
        components: vec![
            "tags".to_string(),
            EMPTY_DIR.to_string(),
            "Premier".to_string(),
            "Second".to_string(),
            FILES_DIR.to_string(),
            "Projet".to_string(),
            "Sous".to_string(),
            "fichier.txt".to_string(),
        ],
    };
    assert_eq!(tags.upload_folder(&app), ["inbox", &date, "Projet", "Sous"]);
    assert_eq!(
        tags.collection_folder(&app),
        ["inbox", &date, "Projet", "Sous", "fichier.txt"]
    );
    assert_eq!(tags.upload_base_len(), 5);

    let direct = Collection {
        kind: CollectionKind::TagFiles { tags: vec![tag] },
        components: vec![
            "tags".to_string(),
            "Premier".to_string(),
            FILES_DIR.to_string(),
        ],
    };
    assert_eq!(direct.upload_folder(&app), ["inbox", &date]);
    assert_eq!(direct.collection_folder(&app), ["inbox", &date]);
    assert_eq!(direct.upload_base_len(), 3);
}

#[test]
fn temp_path_stays_under_dav_hash_components() {
    let (_temp, app, tag, _alias) = fixture();
    let collection = Collection {
        components: vec!["tags".to_string(), "Txt Test".to_string()],
        kind: CollectionKind::TagFiles { tags: vec![tag] },
    };
    let expected = app
        .upload_tmp_dir()
        .join("dav")
        .join(token::hash_token(&collection.key()))
        .join(token::hash_token("note.txt"));
    assert_eq!(temp_path(&app, &collection, "note.txt"), expected);
}

#[test]
fn put_staging_path_uses_dav_label_and_part_suffix() {
    let (_temp, app, _tag, _alias) = fixture();
    let path = put_staging_path(&app, "42").expect("staging");
    assert_eq!(path.parent(), Some(app.upload_tmp_dir().as_path()));
    let name = path.file_name().and_then(OsStr::to_str).expect("nom");
    assert!(name.starts_with("dav-42-"), "{name}");
    assert!(name.ends_with(".part"), "{name}");
}
