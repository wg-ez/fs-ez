//! Application de PUID/PGID aux chemins réels sous la racine puis bascule du processus.

use std::fs;
use std::os::unix::fs::{MetadataExt, lchown};
use std::path::Path;

use fs_ez_core::FileKind;
use fs_ez_fs::PinnedDir;
use nix::unistd::{Gid, Uid, getgid, getuid, setgid, setgroups, setuid};

use crate::app::App;
use crate::config::{FileOwner, file_owner};
use crate::error::{AppError, io_error};

/// Profondeur maximale du walk : borne les descripteurs ouverts par niveau, jamais de cycle.
const MAX_WALK_DEPTH: usize = 512;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct OwnerReport {
    pub applied: u64,
    pub skipped: u64,
    pub failed: u64,
}

impl App {
    /// Chown PUID:PGID récursif sous la racine et dans le dossier d'état.
    pub fn apply_owner_layout(&self) -> Result<OwnerReport, AppError> {
        let owner = file_owner(&self.root)?;
        let mut report = OwnerReport::default();
        if !can_apply(owner) {
            tracing::debug!(uid = owner.uid, gid = owner.gid, "chown ignoré: privilèges");
            return Ok(report);
        }
        let root_path = self.root.path().to_path_buf();
        // Inodes épinglés avant tout chown : un swap de chemin ne redirige plus le walk.
        let root = PinnedDir::open(&root_path)?;
        let state = self.root.pinned_state_dir()?;
        chown_tree(&root, owner, &mut report);
        if state_owner_target(&root_path, &state)? {
            chown_tree(&state, owner, &mut report);
        }
        Ok(report)
    }

    /// Bascule irréversible vers PUID:PGID, root uniquement.
    pub fn drop_privileges(&self) -> Result<Option<FileOwner>, AppError> {
        let owner = file_owner(&self.root)?;
        let uid = getuid();
        let gid = getgid();
        if uid != Uid::from_raw(0) {
            if uid.as_raw() != owner.uid || gid.as_raw() != owner.gid {
                tracing::warn!(
                    uid = uid.as_raw(),
                    gid = gid.as_raw(),
                    puid = owner.uid,
                    pgid = owner.gid,
                    "PUID/PGID ignorés: processus non-root"
                );
            }
            return Ok(None);
        }
        if owner.uid == 0 && owner.gid == 0 {
            return Ok(None);
        }
        // Groupes supplémentaires vidés comme s6-setuidgid, puis gid avant uid.
        let target_gid = Gid::from_raw(owner.gid);
        setgroups(&[target_gid]).map_err(|error| privilege_error("setgroups", error))?;
        setgid(target_gid).map_err(|error| privilege_error("setgid", error))?;
        setuid(Uid::from_raw(owner.uid)).map_err(|error| privilege_error("setuid", error))?;
        let dropped_uid = getuid();
        let dropped_gid = getgid();
        if dropped_uid.as_raw() != owner.uid || dropped_gid.as_raw() != owner.gid {
            return Err(AppError::Invalid(format!(
                "PUID/PGID non appliqués: {}:{}",
                dropped_uid.as_raw(),
                dropped_gid.as_raw()
            )));
        }
        Ok(Some(owner))
    }

    /// Chown du fichier créé par fs-ez, par exemple un upload déplacé dans le dossier files.
    pub fn apply_owner_path(&self, path: &Path) -> Result<(), AppError> {
        let owner = file_owner(&self.root)?;
        if !can_apply(owner) {
            return Ok(());
        }
        // Déjà au bon propriétaire : éviter un lchown par fichier sur les gros scans.
        let meta = fs::symlink_metadata(path).map_err(|error| io_error(path, error))?;
        if meta.uid() == owner.uid && meta.gid() == owner.gid {
            return Ok(());
        }
        lchown(path, Some(owner.uid), Some(owner.gid)).map_err(|error| io_error(path, error))
    }
}

/// Non-root : seulement si PUID/PGID correspondent déjà à l'identité du processus.
fn can_apply(owner: FileOwner) -> bool {
    let Ok(meta) = fs::metadata("/proc/self") else {
        return false;
    };
    meta.uid() == 0 || (meta.uid() == owner.uid && meta.gid() == owner.gid)
}

/// État hors root, décidé sur l'inode épinglé, jamais sur un chemin re-résolu.
fn state_owner_target(root: &Path, state: &PinnedDir) -> Result<bool, AppError> {
    let state = state.real_path()?;
    Ok(!state.starts_with(root))
}

fn chown_tree(root: &PinnedDir, owner: FileOwner, report: &mut OwnerReport) {
    apply_dir(root, owner, report);
    walk(root, owner, report, 0);
}

/// Walk fd-relatif : chaque descente ouvre un enfant du descripteur et vérifie `(dev, ino)`.
fn walk(dir: &PinnedDir, owner: FileOwner, report: &mut OwnerReport, depth: usize) {
    if depth > MAX_WALK_DEPTH {
        report.failed = report.failed.saturating_add(1);
        tracing::warn!(depth, "chown walk depth limit reached");
        return;
    }
    let entries = match dir.read_dir() {
        Ok(entries) => entries,
        Err(error) => {
            report.failed = report.failed.saturating_add(1);
            tracing::warn!(dir = %dir.path().display(), %error, "chown read_dir failed");
            return;
        }
    };
    for entry in entries {
        if matches!(entry.kind, FileKind::Directory) {
            match dir.open_child_dir_checked(&entry.name, entry.dev, entry.ino) {
                Ok(child) => {
                    apply_dir(&child, owner, report);
                    walk(&child, owner, report, depth + 1);
                }
                Err(error) => {
                    report.failed = report.failed.saturating_add(1);
                    tracing::warn!(%error, "chown descend refused");
                }
            }
            continue;
        }
        // Lien ou fichier : lchown relatif au descripteur, jamais la cible du lien.
        match dir.chown_child(&entry.name, owner.uid, owner.gid) {
            Ok(()) => report.applied = report.applied.saturating_add(1),
            Err(error) => {
                report.failed = report.failed.saturating_add(1);
                tracing::warn!(%error, "chown failed");
            }
        }
    }
}

fn apply_dir(dir: &PinnedDir, owner: FileOwner, report: &mut OwnerReport) {
    match dir.chown(owner.uid, owner.gid) {
        Ok(()) => report.applied = report.applied.saturating_add(1),
        Err(error) => {
            report.failed = report.failed.saturating_add(1);
            tracing::warn!(dir = %dir.path().display(), %error, "chown failed");
        }
    }
}

fn privilege_error(step: &str, error: nix::errno::Errno) -> AppError {
    AppError::Invalid(format!("{step}: {error}"))
}

#[cfg(test)]
mod tests;
