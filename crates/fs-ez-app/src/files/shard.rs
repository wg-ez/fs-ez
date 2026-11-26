//! Sharding physique du dossier files : deux niveaux dérivés du nom, nom réel conservé.

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Le nom reste lisible sur disque, le shard se dérive sans la base pour la récupération.
pub fn shard_components(name: &OsStr) -> [String; 2] {
    let digest = Sha256::digest(name.as_bytes());
    [format!("{:02x}", digest[0]), format!("{:02x}", digest[1])]
}

pub fn shard_dir(files: &Path, name: &OsStr) -> PathBuf {
    let [first, second] = shard_components(name);
    files.join(first).join(second)
}

pub fn sharded_path(files: &Path, name: &OsStr) -> PathBuf {
    shard_dir(files, name).join(name)
}

#[cfg(test)]
mod tests;
