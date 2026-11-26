use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use fs_ez_app::{App, EnsureOptions, ViewInput};
use fs_ez_core::{FilterExpr, Sort};
use tempfile::TempDir;

mod common;

use common::{test_app, write};

fn mode_of(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).expect("meta").permissions().mode() & 0o777
}

fn expected_db_path(app: &App) -> PathBuf {
    app.root.default_db()
}

#[test]
fn init_is_idempotent_and_keeps_existing_settings() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    let db_path = temp.path().join("config/fs-ez.db");

    let first = app.init(&db_path).expect("init");
    assert_eq!(first.root, app.root.path().to_path_buf());
    assert_eq!(first.db_path, db_path);
    assert_eq!(first.files, app.root.path().join("files"));
    assert!(first.files.is_dir());
    let inbox = app.root.path().join("inbox");
    assert_eq!(app.inbox_path().expect("inbox"), inbox);
    assert!(inbox.is_dir());
    assert_eq!(mode_of(&inbox), 0o755);
    assert_eq!(mode_of(&app.root.state_dir()), 0o700);

    let defaults = app.store.all_settings().expect("réglages");
    assert_eq!(defaults.len(), 12);

    app.store
        .set_setting("ui.recent_days", "30")
        .expect("écrasement volontaire");
    let second = app.init(&db_path).expect("init idempotent");
    assert_eq!(second, first);
    assert_eq!(app.store.all_settings().expect("réglages").len(), 12);
    assert_eq!(
        app.store
            .get_setting("ui.recent_days")
            .expect("lecture")
            .as_deref(),
        Some("30")
    );
}

#[test]
fn io_paths_prefer_toml_over_settings_and_settings_over_root_default() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);

    assert_eq!(
        app.files_path().expect("défaut"),
        app.root.path().join("files")
    );

    app.set_setting("io.files_dir", "bh").expect("réglage");
    assert_eq!(
        app.files_path().expect("réglage"),
        app.root.path().join("bh")
    );

    std::fs::write(temp.path().join("fs-ez.toml"), "files = \"in\"\n").expect("écriture");
    assert_eq!(app.files_path().expect("toml"), app.root.path().join("in"));
}

#[test]
fn repair_finds_moved_file_by_fingerprint() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    let original = write(&temp, "orig.txt", b"contenu stable");
    let file = app
        .ensure_file(&original, EnsureOptions { force: false })
        .expect("fichier");

    let moved = write(&temp, "sub/orig.txt", b"contenu stable");
    std::fs::remove_file(&original).expect("suppression test");

    let report = app.repair().expect("repair");
    assert_eq!(report.moved, 1);
    assert_eq!(report.updated, 0);
    assert_eq!(report.missing, 0);

    let record = app
        .store
        .get_file(file.id)
        .expect("lecture")
        .expect("connu");
    let moved = std::fs::canonicalize(&moved).expect("canon");
    assert_eq!(record.path, moved);
    assert_eq!(record.name, "orig.txt");
    assert_eq!(
        record.dir,
        moved
            .parent()
            .expect("parent")
            .to_string_lossy()
            .to_string()
    );
}

#[test]
fn repair_updates_metadata_and_reports_missing() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    let path = write(&temp, "a.txt", b"court");
    let file = app
        .ensure_file(&path, EnsureOptions { force: false })
        .expect("fichier");

    write(&temp, "a.txt", b"beaucoup plus long");
    let report = app.repair().expect("repair");
    assert_eq!(report.updated, 1);
    assert_eq!(report.moved, 0);
    assert_eq!(report.missing, 0);
    let record = app
        .store
        .get_file(file.id)
        .expect("lecture")
        .expect("connu");
    assert_eq!(record.size, 18);

    let stable = app.repair().expect("repair stable");
    assert_eq!(stable.updated, 0);
    assert_eq!(stable.moved, 0);
    assert_eq!(stable.missing, 0);

    std::fs::remove_file(&path).expect("suppression test");
    let report = app.repair().expect("repair");
    assert_eq!(report.missing, 1);
    assert_eq!(report.moved, 0);
}

#[test]
fn repair_picks_first_lexical_candidate_and_keeps_symlinks_untouched() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    let path = write(&temp, "lost.txt", "meme empreinte".as_bytes());
    let file = app
        .ensure_file(&path, EnsureOptions { force: false })
        .expect("fichier");
    let canonical = std::fs::canonicalize(&path).expect("canon");

    let outside = TempDir::new().expect("hors root");
    let outside_file = outside.path().join("lien.txt");
    std::fs::write(&outside_file, "meme empreinte".as_bytes()).expect("hors root");
    symlink(&outside_file, temp.path().join("lien.txt")).expect("symlink");
    std::fs::remove_file(&path).expect("suppression test");

    let report = app.repair().expect("repair");
    assert_eq!(report.missing, 1);
    assert_eq!(report.moved, 0);
    let record = app
        .store
        .get_file(file.id)
        .expect("lecture")
        .expect("connu");
    assert_eq!(record.path, canonical);

    write(&temp, "z-copie.txt", "meme empreinte".as_bytes());
    write(&temp, "a-copie.txt", "meme empreinte".as_bytes());
    let report = app.repair().expect("repair");
    assert_eq!(report.moved, 1);
    let record = app
        .store
        .get_file(file.id)
        .expect("lecture")
        .expect("connu");
    assert_eq!(
        record.path,
        std::fs::canonicalize(temp.path().join("a-copie.txt")).expect("canon")
    );
}

#[test]
fn repair_counts_missing_directory() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    let dir = temp.path().join("dossier");
    std::fs::create_dir(&dir).expect("mkdir");
    app.ensure_file(&dir, EnsureOptions { force: false })
        .expect("dossier");

    std::fs::remove_dir(&dir).expect("suppression test");
    let report = app.repair().expect("repair");
    assert_eq!(report.missing, 1);
    assert_eq!(report.moved, 0);
    assert_eq!(report.updated, 0);
}

#[test]
fn status_on_empty_db_and_with_content() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);

    let empty = app.status().expect("statut vide");
    assert_eq!(empty.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(empty.root, app.root.path());
    assert_eq!(empty.db_path, expected_db_path(&app));
    assert_eq!(empty.db_size, 0);
    assert_eq!(empty.files, 0);
    assert_eq!(empty.untagged, 0);
    assert_eq!(empty.recent, 0);
    assert_eq!(empty.tags, 0);
    assert_eq!(empty.views, 0);

    let path = write(&temp, "tagged.txt", b"x");
    app.tag_path(&path, &["Projet".to_string()], false)
        .expect("tag");
    app.create_view(ViewInput {
        name: "Tous".to_string(),
        filter: Some(FilterExpr::Tag {
            tag: "Projet".to_string(),
            negated: false,
        }),
        icon: None,
        color: None,
        position: None,
        sort: Sort::default(),
    })
    .expect("vue");
    let status = app.status().expect("statut");
    assert_eq!(status.files, 1);
    assert_eq!(status.untagged, 0);
    assert_eq!(status.recent, 1);
    assert_eq!(status.tags, 1);
    assert_eq!(status.views, 1);
}

#[test]
fn status_sums_db_wal_and_shm_when_present() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    std::fs::create_dir_all(app.root.state_dir()).expect("état");
    let db_path = app.root.default_db();
    std::fs::write(&db_path, b"12345").expect("db");
    for suffix in ["-wal", "-shm"] {
        let mut raw = db_path.as_os_str().to_os_string();
        raw.push(suffix);
        std::fs::write(PathBuf::from(raw), b"abc").expect("annexe");
    }

    let status = app.status().expect("statut");
    assert_eq!(status.db_path, db_path);
    assert_eq!(status.db_size, 11);
    assert_eq!(status.files, 0);
}

#[test]
fn repair_flushes_metadata_updates_in_batches() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    for index in 0..501 {
        let name = format!("lot/{index:03}.txt");
        let path = write(&temp, &name, format!("court {index:03}").as_bytes());
        app.ensure_file(&path, EnsureOptions { force: false })
            .expect("fichier");
        write(
            &temp,
            &name,
            format!("beaucoup plus long {index:03}").as_bytes(),
        );
    }

    let report = app.repair().expect("repair");

    assert_eq!(report.updated, 501);
    assert_eq!(report.moved, 0);
    assert_eq!(report.missing, 0);
    let stable = app.repair().expect("repair stable");
    assert_eq!(stable.updated, 0);
}

#[test]
fn repair_counts_a_conflicting_write_as_missing() {
    let temp = TempDir::new().expect("tempdir");
    let app = test_app(&temp);
    let lost = write(&temp, "perdu.txt", b"meme contenu");
    let occupied = write(&temp, "occupe.txt", b"autre contenu bien plus long");
    let lost_id = app
        .ensure_file(&lost, EnsureOptions { force: false })
        .expect("perdu")
        .id;
    let occupied_id = app
        .ensure_file(&occupied, EnsureOptions { force: false })
        .expect("occupe")
        .id;
    std::fs::remove_file(&lost).expect("suppression test");
    // Le candidat par empreinte porte le chemin déjà détenu par l'autre ligne.
    std::fs::write(&occupied, b"meme contenu").expect("ecrasement test");

    let report = app.repair().expect("repair");

    assert_eq!(report.moved, 0);
    assert_eq!(report.missing, 1);
    assert_eq!(report.updated, 1);
    let lost_record = app
        .store
        .get_file(lost_id)
        .expect("lecture")
        .expect("connu");
    assert_eq!(
        lost_record.path,
        std::fs::canonicalize(temp.path())
            .expect("canon")
            .join("perdu.txt")
    );
    let occupied_record = app
        .store
        .get_file(occupied_id)
        .expect("lecture")
        .expect("connu");
    assert_eq!(occupied_record.size, 12);
}
