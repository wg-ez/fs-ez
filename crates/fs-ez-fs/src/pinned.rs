//! Dossier épinglé : opérations enfants relatives au descripteur, jamais à un chemin re-résolu.

use std::ffi::OsStr;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt, fchown, lchown};
use std::path::{Component, Path, PathBuf};

use fs_ez_core::FileKind;

use crate::error::FsError;
use crate::system::{DirEntryInfo, kind_of, read_dir_path};

/// Descripteur de dossier `O_NOFOLLOW|O_DIRECTORY` : un renommage ou un lien concurrent ne le redirige pas.
pub struct PinnedDir {
    dir: File,
}

impl PinnedDir {
    /// Ouvre un dossier sans suivre de lien final, un lien est refusé avant toute opération.
    pub fn open(path: &Path) -> Result<Self, FsError> {
        let dir = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY)
            .open(path)
            .map_err(|error| match error.raw_os_error() {
                Some(code) if code == libc::ELOOP || code == libc::ENOTDIR => FsError::Invalid(
                    format!("not a directory: {}", crate::path::display_lossy(path)),
                ),
                _ => FsError::from_io(path, error),
            })?;
        Ok(Self { dir })
    }

    /// Duplicata du descripteur : même inode épinglé, utilisable pour repartir de la racine.
    pub fn try_clone(&self) -> Result<Self, FsError> {
        let dir = self
            .dir
            .try_clone()
            .map_err(|error| FsError::Io(format!("pinned dir: {error}")))?;
        Ok(Self { dir })
    }

    /// Identité `(dev, ino)` du dossier ouvert, lue par `fstat` sur le descripteur.
    pub fn identity(&self) -> Result<(u64, u64), FsError> {
        let meta = self
            .dir
            .metadata()
            .map_err(|error| FsError::Io(format!("pinned dir: {error}")))?;
        Ok((meta.dev(), meta.ino()))
    }

    /// Chemin magique Linux du descripteur, base de toute opération enfant.
    pub fn path(&self) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}", self.dir.as_raw_fd()))
    }

    /// Chemin d'une entrée sous le descripteur, jamais résolu hors de ce dossier.
    pub fn child(&self, name: &OsStr) -> PathBuf {
        let mut path = self.path();
        path.push(name);
        path
    }

    /// Ouvre un sous-dossier sans lien final, la liste reste attachée à l'inode vérifié.
    pub fn open_child_dir(&self, name: &OsStr) -> Result<Self, FsError> {
        Self::open(&self.child(name))
    }

    /// Ouvre un fichier régulier sans lien final, métadonnées lues sur le descripteur.
    pub fn open_child_file(&self, name: &OsStr) -> Result<(File, Metadata), FsError> {
        let path = self.child(name);
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&path)
            .map_err(|error| FsError::from_io(&path, error))?;
        let meta = file
            .metadata()
            .map_err(|error| FsError::from_io(&path, error))?;
        if !meta.file_type().is_file() {
            return Err(FsError::Invalid(format!(
                "not a regular file: {}",
                crate::path::display_lossy(&path)
            )));
        }
        Ok((file, meta))
    }

    /// Type `lstat` d'une entrée sous le descripteur, un lien final est vu comme lien.
    pub fn entry_kind(&self, name: &OsStr) -> Result<FileKind, FsError> {
        let path = self.child(name);
        let meta = fs::symlink_metadata(&path).map_err(|error| FsError::from_io(&path, error))?;
        Ok(kind_of(&meta.file_type()))
    }

    /// Liste le dossier depuis le descripteur vérifié, jamais en le fermant puis rouvrant le chemin.
    pub fn read_dir(&self) -> Result<Vec<DirEntryInfo>, FsError> {
        read_dir_path(&self.path())
    }

    /// Unlink d'une entrée relative au descripteur, un lien final n'est jamais suivi.
    pub fn remove_file(&self, name: &OsStr) -> Result<(), FsError> {
        let path = self.child(name);
        fs::remove_file(&path).map_err(|error| FsError::from_io(&path, error))
    }

    /// `rmdir` d'une entrée relative au descripteur, un lien final est refusé par le noyau.
    pub fn remove_dir(&self, name: &OsStr) -> Result<(), FsError> {
        let path = self.child(name);
        fs::remove_dir(&path).map_err(|error| FsError::from_io(&path, error))
    }

    /// Ouvre un chemin absolu depuis `/`, chaque composant refuse un lien.
    pub fn open_absolute(path: &Path) -> Result<Self, FsError> {
        if !path.is_absolute() {
            return Err(invalid_absolute(path));
        }
        let mut dir = Self::open(Path::new("/"))?;
        for component in path.components() {
            match component {
                Component::RootDir => {}
                Component::Normal(name) => dir = dir.open_child_dir(name)?,
                _ => return Err(invalid_absolute(path)),
            }
        }
        Ok(dir)
    }

    /// Ouvre ou crée un chemin absolu depuis `/`, aucun composant n'est suivi par lien.
    pub fn open_or_create_absolute(path: &Path, mode: u32) -> Result<Self, FsError> {
        if !path.is_absolute() {
            return Err(invalid_absolute(path));
        }
        let mut dir = Self::open(Path::new("/"))?;
        for component in path.components() {
            let name = match component {
                Component::RootDir => continue,
                Component::Normal(name) => name,
                _ => return Err(invalid_absolute(path)),
            };
            dir = match dir.open_child_dir(name) {
                Ok(child) => child,
                Err(FsError::NotFound(_)) => {
                    let child_path = dir.child(name);
                    match fs::create_dir(&child_path) {
                        Ok(()) => {}
                        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                        Err(error) => return Err(FsError::from_io(&child_path, error)),
                    }
                    let child = dir.open_child_dir(name)?;
                    child.set_mode(mode)?;
                    child
                }
                Err(error) => return Err(error),
            };
        }
        Ok(dir)
    }

    /// `fchmod` sur le descripteur, jamais par chemin.
    pub fn set_mode(&self, mode: u32) -> Result<(), FsError> {
        self.dir
            .set_permissions(fs::Permissions::from_mode(mode))
            .map_err(|error| FsError::Io(format!("chmod {}: {error}", self.path().display())))
    }

    /// `fchown` du dossier épinglé, l'inode du descripteur est seul modifié.
    pub fn chown(&self, uid: u32, gid: u32) -> Result<(), FsError> {
        fchown(&self.dir, Some(uid), Some(gid))
            .map_err(|error| FsError::Io(format!("chown {}: {error}", self.path().display())))
    }

    /// `lchown` d'une entrée relative au descripteur, un lien final n'est jamais suivi.
    pub fn chown_child(&self, name: &OsStr, uid: u32, gid: u32) -> Result<(), FsError> {
        let path = self.child(name);
        lchown(&path, Some(uid), Some(gid)).map_err(|error| FsError::from_io(&path, error))
    }

    /// Chemin réel de l'inode épinglé, lu depuis `/proc/self/fd`, jamais re-résolu.
    pub fn real_path(&self) -> Result<PathBuf, FsError> {
        fs::read_link(self.path())
            .map_err(|error| FsError::Io(format!("readlink {}: {error}", self.path().display())))
    }

    /// Ouvre un sous-dossier et exige l'identité `(dev, ino)` capturée au listing.
    pub fn open_child_dir_checked(
        &self,
        name: &OsStr,
        dev: u64,
        ino: u64,
    ) -> Result<Self, FsError> {
        let child = self.open_child_dir(name)?;
        if child.identity()? != (dev, ino) {
            return Err(FsError::Invalid(format!(
                "directory identity changed: {}",
                crate::path::display_lossy(&self.child(name))
            )));
        }
        Ok(child)
    }

    /// Résout un chemin relatif composant par composant, chaque étage refusant un lien.
    pub fn open_at(&self, relative: &Path) -> Result<Self, FsError> {
        let mut dir = self.try_clone()?;
        for component in relative.components() {
            match component {
                std::path::Component::Normal(name) => dir = dir.open_child_dir(name)?,
                std::path::Component::CurDir => {}
                _ => {
                    return Err(FsError::OutsideRoot(format!(
                        "relative path: {}",
                        crate::path::display_lossy(relative)
                    )));
                }
            }
        }
        Ok(dir)
    }
}

fn invalid_absolute(path: &Path) -> FsError {
    FsError::Invalid(format!(
        "invalid absolute path: {}",
        crate::path::display_lossy(path)
    ))
}
