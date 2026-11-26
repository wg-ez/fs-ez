use std::fs;

use common::Harness;
use time::OffsetDateTime;
use time::macros::format_description;

mod common;

const STAMP: &[time::format_description::FormatItem<'static>] =
    format_description!("[year][month][day]-[hour][minute][second]");

#[test]
fn backup_creates_a_copy_and_prunes_old_ones() {
    let harness = Harness::new();
    harness
        .app
        .set_setting("io.backup_days", "1")
        .expect("fenêtre");
    let dir = harness.app.backup_dir();
    fs::create_dir_all(&dir).expect("dossier");
    let old = dir.join("fs-ez-20200101-000000.db");
    fs::write(&old, b"old").expect("ancienne copie");
    let stamp = OffsetDateTime::now_utc().format(STAMP).expect("stamp");
    let recent = dir.join(format!("fs-ez-{stamp}.db"));
    fs::write(&recent, b"recent").expect("copie récente");
    let foreign = dir.join("notes.txt");
    fs::write(&foreign, b"keep").expect("fichier étranger");

    let report = harness.app.backup().expect("sauvegarde");
    assert!(report.created.is_file());
    assert_eq!(report.pruned, 1);
    assert!(!old.exists());
    assert!(recent.exists());
    assert!(foreign.exists());
}
