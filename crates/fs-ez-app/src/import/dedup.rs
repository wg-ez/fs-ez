//! Vérité des doublons : empreinte intégrale ou comparaison octet par octet.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use fs_ez_core::Fingerprint;

use super::*;
use crate::error::io_error;

/// Issue d'une collision d'empreinte.
pub(crate) enum DuplicateResolution {
    /// Même contenu : la ligne existante est le doublon.
    Duplicate,
    /// Bords communs, milieu différent : identité à enregistrer sous cette empreinte intégrale.
    Distinct(Fingerprint),
}

/// Empreinte face à la base : libre ou déjà possédée par une ligne confirmée.
pub(crate) enum FingerprintResolution {
    /// Aucun propriétaire confirmé : à enregistrer telle quelle.
    Available(Fingerprint),
    /// Contenu identique détenu par cette ligne.
    Duplicate(FileRecord),
}

/// Verdict d'une comparaison de contenu.
#[derive(PartialEq, Eq)]
enum DuplicateCheck {
    Same,
    Different,
}

impl App {
    /// Vrai si l'empreinte couvre tout le fichier, donc comparable sans relire les octets.
    pub(crate) fn fingerprint_covers_file(&self, size: u64) -> bool {
        size <= self.fingerprints.full_hash_threshold()
    }

    /// Tranche une empreinte commune : doublon si tout le contenu est identique, sinon empreinte intégrale.
    pub(crate) fn resolve_duplicate(
        &self,
        candidate: &Path,
        existing: &Path,
        fingerprint_complete: bool,
    ) -> Result<DuplicateResolution, AppError> {
        if self.check_duplicate(candidate, existing, fingerprint_complete)? == DuplicateCheck::Same
        {
            return Ok(DuplicateResolution::Duplicate);
        }
        Ok(DuplicateResolution::Distinct(
            self.fingerprint_complete(candidate)?,
        ))
    }

    /// Résout une empreinte face à la base : libre, doublon confirmé, ou intégrale si collision.
    pub(crate) fn resolve_fingerprint(
        &self,
        candidate: &Path,
        fingerprint: Fingerprint,
        fingerprint_complete: bool,
    ) -> Result<FingerprintResolution, AppError> {
        let Some(existing) = self.store.find_file_by_fingerprint(&fingerprint)? else {
            return Ok(FingerprintResolution::Available(fingerprint));
        };
        if existing.path.as_path() == candidate {
            return Ok(FingerprintResolution::Available(fingerprint));
        }
        match self.resolve_duplicate(candidate, &existing.path, fingerprint_complete)? {
            DuplicateResolution::Duplicate => Ok(FingerprintResolution::Duplicate(existing)),
            DuplicateResolution::Distinct(full) => {
                match self.store.find_file_by_fingerprint(&full)? {
                    Some(owner) if owner.path.as_path() != candidate => {
                        Ok(FingerprintResolution::Duplicate(owner))
                    }
                    _ => Ok(FingerprintResolution::Available(full)),
                }
            }
        }
    }

    /// Intégral sous le seuil, sinon comparaison octet par octet avec la ligne existante.
    fn check_duplicate(
        &self,
        candidate: &Path,
        existing: &Path,
        fingerprint_complete: bool,
    ) -> Result<DuplicateCheck, AppError> {
        if fingerprint_complete {
            return Ok(DuplicateCheck::Same);
        }
        let stored = self.root.confine_existing(existing)?;
        if same_content(candidate, &stored)? {
            Ok(DuplicateCheck::Same)
        } else {
            Ok(DuplicateCheck::Different)
        }
    }

    /// Empreinte sur tout le contenu : identité distincte de deux gros fichiers qui partagent leurs bords.
    fn fingerprint_complete(&self, path: &Path) -> Result<Fingerprint, AppError> {
        Ok(self.fingerprints.fingerprint_complete(path)?)
    }
}

/// Comparaison intégrale en flux, taille d'abord puis blocs de 64 Kio.
fn same_content(left: &Path, right: &Path) -> Result<bool, AppError> {
    let left_meta = std::fs::metadata(left).map_err(|error| io_error(left, error))?;
    let right_meta = std::fs::metadata(right).map_err(|error| io_error(right, error))?;
    if !left_meta.is_file() || !right_meta.is_file() || left_meta.len() != right_meta.len() {
        return Ok(false);
    }
    let mut left_reader = BufReader::new(File::open(left).map_err(|error| io_error(left, error))?);
    let mut right_reader =
        BufReader::new(File::open(right).map_err(|error| io_error(right, error))?);
    let mut left_buffer = [0u8; 64 * 1024];
    let mut right_buffer = [0u8; 64 * 1024];
    loop {
        let left_len = read_chunk(&mut left_reader, &mut left_buffer)
            .map_err(|error| io_error(left, error))?;
        let right_len = read_chunk(&mut right_reader, &mut right_buffer)
            .map_err(|error| io_error(right, error))?;
        if left_len != right_len || left_buffer[..left_len] != right_buffer[..right_len] {
            return Ok(false);
        }
        if left_len == 0 {
            return Ok(true);
        }
    }
}

fn read_chunk(reader: &mut impl Read, buffer: &mut [u8]) -> std::io::Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        match reader.read(&mut buffer[filled..])? {
            0 => break,
            read => filled += read,
        }
    }
    Ok(filled)
}
