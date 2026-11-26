#![forbid(unsafe_code)]
//! Empreintes de fichiers, dossiers et liens (blake3).

pub mod blake3_impl;
pub mod error;

use std::path::Path;

use fs_ez_core::{FileKind, Fingerprint};

pub use blake3_impl::{Blake3Fingerprinter, DEFAULT_SAMPLE_SIZE, DEFAULT_SAMPLE_THRESHOLD};
pub use error::FingerprintError;

pub trait Fingerprinter: Send + Sync {
    fn fingerprint(&self, path: &Path, kind: FileKind) -> Result<Fingerprint, FingerprintError>;
    /// Empreinte intégrale, distincte de l'échantillon : lève une collision de bords sans perdre de fichier.
    fn fingerprint_complete(&self, path: &Path) -> Result<Fingerprint, FingerprintError>;
    /// Taille sous laquelle l'empreinte couvre tout le fichier, 0 = inconnu donc comparaison intégrale obligatoire.
    fn full_hash_threshold(&self) -> u64 {
        0
    }
}

#[cfg(test)]
mod layout_test;
