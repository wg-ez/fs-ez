//! Sauvegarde de la base et rotation par ancienneté.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use fs_ez_core::datetime::now_local;
use fs_ez_fs::FsError;
use time::macros::format_description;
use time::{Duration, PrimitiveDateTime};

use crate::app::App;
use crate::error::AppError;

const BACKUP_DIR: &str = "backup";
const BACKUP_PREFIX: &str = "fs-ez-";
const BACKUP_SUFFIX: &str = ".db";
const STAMP_LEN: usize = 15;
const STAMP: &[time::format_description::FormatItem<'static>] =
    format_description!("[year][month][day]-[hour][minute][second]");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupReport {
    pub created: PathBuf,
    pub pruned: usize,
}

impl App {
    /// Dossier de sauvegarde, sous le dossier d'état (`/config` en Docker).
    pub fn backup_dir(&self) -> PathBuf {
        self.root.state_dir().join(BACKUP_DIR)
    }

    /// Une copie maintenant, les copies plus anciennes que `io.backup_days` sont supprimées.
    pub fn backup(&self) -> Result<BackupReport, AppError> {
        let settings = self.settings()?;
        let dir = self.backup_dir();
        fs::create_dir_all(&dir).map_err(|error| io_error(&dir, error))?;
        // Les copies contiennent toute la base : dossier privé, échec de permission ignoré.
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
        let target = unique_target(&dir);
        self.store.backup_to(&target)?;
        let _ = fs::set_permissions(&target, fs::Permissions::from_mode(0o600));
        let pruned = prune(&dir, settings.backup_days);
        Ok(BackupReport {
            created: target,
            pruned,
        })
    }
}

fn io_error(path: &Path, error: std::io::Error) -> AppError {
    AppError::Fs(FsError::Io(format!("{}: {error}", path.display())))
}

fn unique_target(dir: &Path) -> PathBuf {
    let stamp = now_local()
        .format(STAMP)
        .unwrap_or_else(|_| "backup".to_string());
    let mut candidate = dir.join(format!("{BACKUP_PREFIX}{stamp}{BACKUP_SUFFIX}"));
    let mut suffix = 1usize;
    while candidate.exists() {
        candidate = dir.join(format!("{BACKUP_PREFIX}{stamp}-{suffix}{BACKUP_SUFFIX}"));
        suffix += 1;
    }
    candidate
}

/// Date encodée dans le nom (heure locale du processus), jamais `mtime` : une copie déplacée garde son âge réel.
fn backup_time(path: &Path) -> Option<PrimitiveDateTime> {
    let name = path.file_name()?.to_str()?;
    let raw = name
        .strip_prefix(BACKUP_PREFIX)?
        .strip_suffix(BACKUP_SUFFIX)?;
    let stamp = raw.get(..STAMP_LEN)?;
    let rest = raw.get(STAMP_LEN..)?;
    if !(rest.is_empty() || rest.starts_with('-')) {
        return None;
    }
    PrimitiveDateTime::parse(stamp, STAMP).ok()
}

/// Ne supprime que les fichiers produits par fs-ez et plus vieux que la fenêtre.
fn prune(dir: &Path, days: i64) -> usize {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    let cutoff = now_local() - Duration::days(days);
    let offset = cutoff.offset();
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| backup_time(path).is_some_and(|time| time.assume_offset(offset) < cutoff))
        .filter(|path| fs::remove_file(path).is_ok())
        .count()
}
