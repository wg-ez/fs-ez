//! Noms des fichiers temporaires d'upload, DAV et remplacement atomique.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Préfixe des temporaires d'upload, encore reconnu par la purge.
pub const UPLOAD_PREFIX: &str = "upload-";
/// Préfixe des staging PUT DAV.
pub const DAV_PREFIX: &str = "dav-";
/// Suffixe commun des temporaires reconnus par la purge.
pub const PART_SUFFIX: &str = ".part";
/// Répertoire des temporaires DAV hashés sous `upload_tmp_dir`.
pub const DAV_DIR: &str = "dav";
/// Longueur hexadécimale d'un SHA-256 utilisé comme nom de dossier ou de fichier.
pub const HASH_LEN: usize = 64;
/// Préfixe des remplacements atomiques de contenu, hors purge aujourd'hui.
pub const CONTENT_PREFIX: &str = ".dav-";

/// `dav-<label>-<token>.part`, staging d'un PUT DAV.
pub fn dav_part(label: &str, token: &str) -> String {
    format!("{DAV_PREFIX}{label}-{token}{PART_SUFFIX}")
}

/// `.dav-<token>.part`, remplacement atomique dans le dossier cible.
pub fn content_part(token: &str) -> String {
    format!("{CONTENT_PREFIX}{token}{PART_SUFFIX}")
}

/// `dav/<hash-collection>/<hash-nom>`, relatif à `upload_tmp_dir`.
pub fn dav_hash_path(collection_hash: &str, name_hash: &str) -> PathBuf {
    Path::new(DAV_DIR).join(collection_hash).join(name_hash)
}

/// Nom reconnu par `purge_upload_temps` : préfixe `upload-` ou `dav-`, suffixe `.part`.
pub fn is_temp_file_name(name: &OsStr) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };
    name.ends_with(PART_SUFFIX) && (name.starts_with(UPLOAD_PREFIX) || name.starts_with(DAV_PREFIX))
}

/// Nom hashé accepté par la purge DAV : 64 caractères hexadécimaux minuscules.
pub fn is_hash_name(name: &OsStr) -> bool {
    name.to_str().is_some_and(|name| {
        name.len() == HASH_LEN
            && name
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    })
}

#[cfg(test)]
mod tests;
