//! Implémentation blake3 : hachage complet, échantillonné et structurel.

use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;

use fs_ez_core::{FileKind, Fingerprint};

use crate::Fingerprinter;
use crate::error::FingerprintError;

/// Seuil par défaut avant échantillonnage : 16 Mio.
pub const DEFAULT_SAMPLE_THRESHOLD: u64 = 16 * 1024 * 1024;
/// Taille par défaut de chaque extrémité échantillonnée : 1 Mio.
pub const DEFAULT_SAMPLE_SIZE: u64 = 1024 * 1024;

const CHUNK: usize = 64 * 1024;
const DOMAIN: &[u8] = b"fs-ez-blake3-v1";
const PREFIX: &str = "blake3:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Blake3Fingerprinter {
    pub sample_threshold: u64,
    pub sample_size: u64,
}

impl Default for Blake3Fingerprinter {
    fn default() -> Self {
        Self {
            sample_threshold: DEFAULT_SAMPLE_THRESHOLD,
            sample_size: DEFAULT_SAMPLE_SIZE,
        }
    }
}

impl Blake3Fingerprinter {
    pub fn new(sample_threshold: u64, sample_size: u64) -> Result<Self, FingerprintError> {
        let fingerprinter = Self {
            sample_threshold,
            sample_size,
        };
        fingerprinter.validate()?;
        Ok(fingerprinter)
    }

    /// Les champs sont publics : revalider avant chaque calcul évite un seek invalide.
    fn validate(&self) -> Result<(), FingerprintError> {
        if self.sample_threshold == 0
            || self.sample_size == 0
            || self.sample_size > self.sample_threshold
            || self.sample_size > i64::MAX as u64
            || self.sample_size > u64::MAX / 2
        {
            return Err(FingerprintError::Config);
        }
        Ok(())
    }
}

impl Fingerprinter for Blake3Fingerprinter {
    fn fingerprint(&self, path: &Path, kind: FileKind) -> Result<Fingerprint, FingerprintError> {
        self.validate()?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(DOMAIN);
        match kind {
            FileKind::File => {
                hasher.update(b"file");
                hash_file(path, self.sample_threshold, self.sample_size, &mut hasher)?;
            }
            FileKind::Directory => {
                hasher.update(b"dir");
                hash_dir(path, &mut hasher)?;
            }
            FileKind::Symlink => {
                hasher.update(b"symlink");
                hash_symlink(path, &mut hasher)?;
            }
        }
        let hex = hasher.finalize().to_hex();
        Fingerprint::new(format!("{PREFIX}{hex}"))
            .map_err(|error| FingerprintError::Io(error.to_string()))
    }

    fn fingerprint_complete(&self, path: &Path) -> Result<Fingerprint, FingerprintError> {
        self.validate()?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(DOMAIN);
        hasher.update(b"file-full");
        let mut file = open_regular(path)?;
        hash_bytes(path, &mut file, None, &mut hasher)?;
        let hex = hasher.finalize().to_hex();
        Fingerprint::new(format!("{PREFIX}{hex}"))
            .map_err(|error| FingerprintError::Io(error.to_string()))
    }

    fn full_hash_threshold(&self) -> u64 {
        self.sample_threshold
    }
}

/// Fichier régulier ouvert sans lien final, type vérifié sur le descripteur.
fn open_regular(path: &Path) -> Result<File, FingerprintError> {
    // Types spéciaux refusés avant toute ouverture : un FIFO ouvert en lecture bloquerait.
    let meta = fs::symlink_metadata(path).map_err(|error| FingerprintError::io(path, error))?;
    if !meta.file_type().is_file() {
        return Err(FingerprintError::unsupported(path));
    }
    // O_NOFOLLOW + O_NONBLOCK : un lien ou un FIFO substitué entre deux appels ne bloque pas.
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|error| FingerprintError::io(path, error))?;
    let meta = file
        .metadata()
        .map_err(|error| FingerprintError::io(path, error))?;
    if !meta.file_type().is_file() {
        return Err(FingerprintError::unsupported(path));
    }
    Ok(file)
}

fn hash_file(
    path: &Path,
    sample_threshold: u64,
    sample_size: u64,
    hasher: &mut blake3::Hasher,
) -> Result<(), FingerprintError> {
    let mut file = open_regular(path)?;
    let size = file
        .metadata()
        .map_err(|error| FingerprintError::io(path, error))?
        .len();

    // Sous le seuil, ou trop court pour distinguer les deux extrémités : lecture complète.
    if size <= sample_threshold || size < sample_size * 2 {
        return hash_bytes(path, &mut file, None, hasher);
    }

    hasher.update(&size.to_le_bytes());
    hash_bytes(path, &mut file, Some(sample_size), hasher)?;
    let offset = i64::try_from(sample_size).map_err(|_| FingerprintError::Config)?;
    file.seek(SeekFrom::End(-offset))
        .map_err(|error| FingerprintError::io(path, error))?;
    hash_bytes(path, &mut file, Some(sample_size), hasher)
}

/// Hache jusqu'à EOF (`None`) ou exactement `limit` octets, EOF prématuré refusé.
fn hash_bytes(
    path: &Path,
    file: &mut File,
    limit: Option<u64>,
    hasher: &mut blake3::Hasher,
) -> Result<(), FingerprintError> {
    let mut buffer = [0u8; CHUNK];
    let mut remaining = limit;
    loop {
        let want = match remaining {
            Some(0) => return Ok(()),
            Some(left) => {
                usize::try_from(left.min(CHUNK as u64)).map_err(|_| FingerprintError::Config)?
            }
            None => CHUNK,
        };
        let read = file
            .read(&mut buffer[..want])
            .map_err(|error| FingerprintError::io(path, error))?;
        if read == 0 {
            return match remaining {
                Some(_) => Err(FingerprintError::io(
                    path,
                    io::Error::from(io::ErrorKind::UnexpectedEof),
                )),
                None => Ok(()),
            };
        }
        hasher.update(&buffer[..read]);
        if let Some(left) = remaining.as_mut() {
            *left -= read as u64;
        }
    }
}

struct DirEntryHash {
    name: Vec<u8>,
    label: u8,
    size: u64,
    mtime_seconds: i64,
    mtime_nanos: u32,
}

/// Un seul niveau est hashé : le contenu récursif est ignoré pour borner le coût.
fn hash_dir(path: &Path, hasher: &mut blake3::Hasher) -> Result<(), FingerprintError> {
    let meta = fs::symlink_metadata(path).map_err(|error| FingerprintError::io(path, error))?;
    if !meta.file_type().is_dir() {
        return Err(FingerprintError::unsupported(path));
    }

    let mut entries = Vec::new();
    let read_dir = fs::read_dir(path).map_err(|error| FingerprintError::io(path, error))?;
    for entry in read_dir {
        let entry = entry.map_err(|error| FingerprintError::io(path, error))?;
        let entry_path = entry.path();
        let meta = fs::symlink_metadata(&entry_path)
            .map_err(|error| FingerprintError::io(&entry_path, error))?;
        entries.push(DirEntryHash {
            name: entry.file_name().as_bytes().to_vec(),
            label: kind_label(&meta.file_type()),
            size: meta.len(),
            mtime_seconds: meta.mtime(),
            mtime_nanos: u32::try_from(meta.mtime_nsec()).unwrap_or(0),
        });
    }
    entries.sort_by(|left, right| left.name.cmp(&right.name));

    for entry in entries {
        hasher.update(&(entry.name.len() as u64).to_le_bytes());
        hasher.update(&entry.name);
        hasher.update(&[entry.label]);
        hasher.update(&entry.size.to_le_bytes());
        hasher.update(&entry.mtime_seconds.to_le_bytes());
        hasher.update(&entry.mtime_nanos.to_le_bytes());
    }
    Ok(())
}

fn hash_symlink(path: &Path, hasher: &mut blake3::Hasher) -> Result<(), FingerprintError> {
    let meta = fs::symlink_metadata(path).map_err(|error| FingerprintError::io(path, error))?;
    if !meta.file_type().is_symlink() {
        return Err(FingerprintError::unsupported(path));
    }
    // `read_link` ne résout pas la cible : un lien cassé reste hashable.
    let target = fs::read_link(path).map_err(|error| FingerprintError::io(path, error))?;
    hasher.update(target.as_os_str().as_bytes());
    Ok(())
}

fn kind_label(file_type: &fs::FileType) -> u8 {
    if file_type.is_dir() {
        b'd'
    } else if file_type.is_symlink() {
        b'l'
    } else if file_type.is_file() {
        b'f'
    } else {
        b'?'
    }
}

#[cfg(test)]
mod tests;
