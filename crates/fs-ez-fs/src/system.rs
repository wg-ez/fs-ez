//! Opérations FS réelles, derrière le trait `Fs`.

use std::ffi::OsString;
use std::fs::{self, DirBuilder, FileType};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use fs_ez_core::FileKind;
use time::OffsetDateTime;

use crate::error::FsError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMeta {
    pub size: u64,
    pub mtime: OffsetDateTime,
    pub kind: FileKind,
    /// FIFO, socket, device : jamais importable.
    pub is_special: bool,
}

/// Entrée de dossier avec son identité lstat capturée au listing, jamais celle d'une cible de lien.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntryInfo {
    pub name: OsString,
    pub kind: FileKind,
    pub dev: u64,
    pub ino: u64,
}

pub trait Fs: Send + Sync {
    fn metadata(&self, path: &Path) -> Result<FileMeta, FsError>;
    fn canonicalize(&self, path: &Path) -> Result<PathBuf, FsError>;
    fn read_dir(&self, path: &Path) -> Result<Vec<DirEntryInfo>, FsError>;
    /// Liste un dossier après ouverture sans lien et contrôle du (dev, ino) capturé au listing.
    fn read_dir_checked(
        &self,
        path: &Path,
        dev: u64,
        ino: u64,
    ) -> Result<Vec<DirEntryInfo>, FsError>;
    fn read_link(&self, path: &Path) -> Result<PathBuf, FsError>;
    fn read_file(&self, path: &Path) -> Result<Vec<u8>, FsError>;
    fn create_dir_all(&self, path: &Path) -> Result<(), FsError>;
    fn remove_file(&self, path: &Path) -> Result<(), FsError>;
    /// Lien matériel atomique, `AlreadyExists` si `to` est pris, jamais de remplacement.
    fn link_noreplace(&self, from: &Path, to: &Path) -> Result<(), FsError>;
    /// Copie vers `to` créé exclusivement, la cible partielle est retirée en cas d'échec.
    fn copy_exclusive(&self, from: &Path, to: &Path) -> Result<(), FsError>;
    fn copy_file(&self, from: &Path, to: &Path) -> Result<(), FsError>;
    fn set_mtime(&self, path: &Path, mtime: OffsetDateTime) -> Result<(), FsError>;
    fn writable(&self, path: &Path) -> bool;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemFs;

impl Fs for SystemFs {
    fn metadata(&self, path: &Path) -> Result<FileMeta, FsError> {
        // `symlink_metadata` : le type exposé est celui du lien, jamais sa cible.
        let meta = fs::symlink_metadata(path).map_err(|e| FsError::from_io(path, e))?;
        let file_type = meta.file_type();
        let kind = kind_of(&file_type);
        let size = if kind == FileKind::Directory {
            0
        } else {
            meta.len()
        };
        let mtime = meta.modified().map_err(|e| FsError::from_io(path, e))?;
        Ok(FileMeta {
            size,
            mtime: OffsetDateTime::from(mtime),
            kind,
            is_special: is_special(&file_type),
        })
    }

    fn canonicalize(&self, path: &Path) -> Result<PathBuf, FsError> {
        fs::canonicalize(path).map_err(|e| FsError::from_io(path, e))
    }

    fn read_dir(&self, path: &Path) -> Result<Vec<DirEntryInfo>, FsError> {
        read_dir_path(path)
    }

    fn read_dir_checked(
        &self,
        path: &Path,
        dev: u64,
        ino: u64,
    ) -> Result<Vec<DirEntryInfo>, FsError> {
        // L'identité est comparée sur le descripteur ouvert, la liste sort du même inode.
        let dir = crate::pinned::PinnedDir::open(path)?;
        if dir.identity()? != (dev, ino) {
            return Err(FsError::Invalid(format!(
                "directory identity changed: {}",
                crate::path::display_lossy(path)
            )));
        }
        dir.read_dir()
    }

    fn read_link(&self, path: &Path) -> Result<PathBuf, FsError> {
        fs::read_link(path).map_err(|e| FsError::from_io(path, e))
    }

    fn read_file(&self, path: &Path) -> Result<Vec<u8>, FsError> {
        fs::read(path).map_err(|e| FsError::from_io(path, e))
    }

    fn create_dir_all(&self, path: &Path) -> Result<(), FsError> {
        let mut builder = DirBuilder::new();
        builder.recursive(true).mode(0o755);
        builder.create(path).map_err(|e| FsError::from_io(path, e))
    }

    fn remove_file(&self, path: &Path) -> Result<(), FsError> {
        let meta = fs::symlink_metadata(path).map_err(|e| FsError::from_io(path, e))?;
        if meta.file_type().is_dir() {
            return Err(FsError::Invalid(format!(
                "not a file: {}",
                crate::path::display_lossy(path)
            )));
        }
        fs::remove_file(path).map_err(|e| FsError::from_io(path, e))
    }

    fn link_noreplace(&self, from: &Path, to: &Path) -> Result<(), FsError> {
        fs::hard_link(from, to).map_err(|e| FsError::from_io(to, e))
    }

    fn copy_exclusive(&self, from: &Path, to: &Path) -> Result<(), FsError> {
        // O_NOFOLLOW : un lien final substitué à la source n'est jamais lu.
        let mut input = fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(from)
            .map_err(|e| FsError::from_io(from, e))?;
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(to)
            .map_err(|e| FsError::from_io(to, e))?;
        if let Err(error) =
            std::io::copy(&mut input, &mut output).map_err(|e| FsError::from_io(to, e))
        {
            drop(output);
            let _ = fs::remove_file(to);
            return Err(error);
        }
        Ok(())
    }

    fn copy_file(&self, from: &Path, to: &Path) -> Result<(), FsError> {
        fs::copy(from, to).map_err(|e| FsError::from_io(to, e))?;
        Ok(())
    }

    fn set_mtime(&self, path: &Path, mtime: OffsetDateTime) -> Result<(), FsError> {
        let file = fs::OpenOptions::new()
            .write(true)
            .open(path)
            .map_err(|e| FsError::from_io(path, e))?;
        file.set_times(fs::FileTimes::new().set_modified(mtime.into()))
            .map_err(|e| FsError::from_io(path, e))
    }

    fn writable(&self, path: &Path) -> bool {
        // Droit d'écriture du parent : tester le fichier lui-même exigerait de le créer.
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(path);
        match fs::metadata(parent) {
            Ok(meta) => meta.permissions().mode() & 0o200 != 0,
            Err(_) => false,
        }
    }
}

/// Liste un dossier par chemin, identité `lstat` de chaque entrée, tri stable par octets.
pub(crate) fn read_dir_path(path: &Path) -> Result<Vec<DirEntryInfo>, FsError> {
    let entries = fs::read_dir(path).map_err(|e| FsError::from_io(path, e))?;
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| FsError::from_io(path, e))?;
        let file_type = entry.file_type().map_err(|e| FsError::from_io(path, e))?;
        // `DirEntry::metadata` est un lstat : l'identité d'un lien reste celle du lien.
        let (dev, ino) = match entry.metadata() {
            Ok(meta) => (meta.dev(), meta.ino()),
            // Entrée disparue entre readdir et lstat : identité nulle, jamais reconnue ensuite.
            Err(_) => (0, 0),
        };
        out.push(DirEntryInfo {
            name: entry.file_name(),
            kind: kind_of(&file_type),
            dev,
            ino,
        });
    }
    out.sort_by(|a, b| {
        a.name
            .as_os_str()
            .as_bytes()
            .cmp(b.name.as_os_str().as_bytes())
    });
    Ok(out)
}

pub(crate) fn kind_of(file_type: &FileType) -> FileKind {
    if file_type.is_symlink() {
        FileKind::Symlink
    } else if file_type.is_dir() {
        FileKind::Directory
    } else {
        FileKind::File
    }
}

fn is_special(file_type: &FileType) -> bool {
    file_type.is_fifo()
        || file_type.is_socket()
        || file_type.is_block_device()
        || file_type.is_char_device()
}

#[cfg(test)]
mod tests;
