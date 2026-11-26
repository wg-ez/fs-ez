//! Fichiers référencés : type, empreinte et enregistrements.

use std::path::PathBuf;

use serde::{Deserialize, Serialize, Serializer};
use time::OffsetDateTime;

use crate::codec::token_enum;
use crate::error::CoreError;
use crate::ids::FileId;

/// Longueur maximale d'une empreinte en octets.
const FINGERPRINT_MAX_LEN: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    File,
    Directory,
    Symlink,
}

token_enum!(ref FileKind, as_str, parse,
    File => "file",
    Directory => "directory",
    Symlink => "symlink",
);
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Fingerprint(String);

impl Fingerprint {
    pub fn new(raw: impl Into<String>) -> Result<Self, CoreError> {
        let trimmed = raw.into().trim().to_string();
        if trimmed.is_empty() {
            return Err(CoreError::InvalidFingerprint("empty".to_string()));
        }
        if trimmed.len() > FINGERPRINT_MAX_LEN {
            return Err(CoreError::InvalidFingerprint(format!(
                "too long: {} bytes",
                trimmed.len()
            )));
        }
        Ok(Self(trimmed))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewFile {
    pub path: PathBuf,
    pub path_display: String,
    pub name: String,
    pub dir: String,
    pub size: u64,
    pub mtime: OffsetDateTime,
    pub kind: FileKind,
    pub fingerprint: Option<Fingerprint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilePatch {
    pub path: Option<PathBuf>,
    pub path_display: Option<String>,
    pub name: Option<String>,
    pub dir: Option<String>,
    pub size: Option<u64>,
    pub mtime: Option<OffsetDateTime>,
    pub kind: Option<FileKind>,
    /// Outer `None` = inchangé, `Some(None)` = effacer l'empreinte.
    pub fingerprint: Option<Option<Fingerprint>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileRecord {
    pub id: FileId,
    #[serde(serialize_with = "serialize_path_lossy")]
    pub path: PathBuf,
    pub path_display: String,
    pub name: String,
    pub dir: String,
    pub size: u64,
    pub mtime: OffsetDateTime,
    pub kind: FileKind,
    pub fingerprint: Option<Fingerprint>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

/// Un chemin non-UTF8 ne doit jamais faire échouer une sortie JSON, perte assumée.
#[allow(clippy::ptr_arg)] // serde impose exactement `&PathBuf` pour `serialize_with`.
fn serialize_path_lossy<S>(path: &PathBuf, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&path.to_string_lossy())
}

#[cfg(test)]
mod tests;
