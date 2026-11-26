//! Placement shardé des uploads, purge des temporaires et revendication de destination.

use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use time::{Duration, OffsetDateTime};

use fs_ez_core::FileKind;

use super::dedup::DuplicateResolution;
use super::naming::{random_suffix, sanitize_filename, suffixed_name};
use super::*;
use crate::files::sharded_path;
use crate::staging::{DAV_DIR, is_hash_name, is_temp_file_name};

const COLLISION_RETRIES: u32 = 5;

const UPLOAD_TEMP_MAX_AGE: Duration = Duration::hours(24);

/// Nature de la destination revendiquée par `claim_destination`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DestinationKind {
    /// Nom propre demandé, seul cas où un doublon suffixé lui cède la ligne.
    Requested,
    /// Repli `-<6 hex>` sur collision de nom.
    Suffixed,
}

/// Destination posée sur disque et nom propre demandé, avant indexation.
struct PlacedUpload {
    destination: PathBuf,
    requested: String,
    kind: DestinationKind,
}

/// Coût par fichier évité pendant un scan : shards créés, chownés et canonicalisés une fois.
#[derive(Default)]
pub(super) struct PlacementState {
    created: HashSet<PathBuf>,
    owned: HashSet<PathBuf>,
    shards: HashMap<PathBuf, PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Placement {
    /// Lien matériel : mtime et inode partagés avec la source.
    Linked,
    /// Copie exclusive : mtime à restaurer avant indexation.
    Copied,
}

impl App {
    /// Placement shardé puis enregistrement bibliothèque, `temp` consommé en cas de succès.
    pub fn import_upload(
        &self,
        temp: &Path,
        original_name: &str,
        mtime: Option<OffsetDateTime>,
    ) -> Result<LibraryOutcome, AppError> {
        let placed = self.place_upload(temp, original_name, mtime)?;
        // Upload : un nom propre prime sur une copie suffixée du même contenu.
        let outcome = match placed.kind {
            DestinationKind::Requested => {
                self.index_path_with_fingerprint(&placed.destination, None, Some(&placed.requested))
            }
            DestinationKind::Suffixed => self.index_suffixed_upload(&placed),
        };
        match outcome {
            Ok(value) if matches!(value, LibraryOutcome::Duplicate { .. }) => {
                // Copie d'upload : jamais un fichier utilisateur, suppression sûre.
                self.remove_quiet(&placed.destination, "library duplicate cleanup failed");
                Ok(value)
            }
            Ok(value) => Ok(value),
            Err(error) => {
                if let Err(cleanup) = self.fs.remove_file(&placed.destination) {
                    tracing::warn!(path = %display_lossy(&placed.destination), %cleanup, "library rollback failed");
                }
                Err(error)
            }
        }
    }

    /// Copie suffixée : quand le nom propre porte déjà le même contenu, elle ne crée aucune ligne.
    fn index_suffixed_upload(&self, placed: &PlacedUpload) -> Result<LibraryOutcome, AppError> {
        let files = self.files_path()?;
        let clean = sharded_path(&files, OsStr::new(&placed.requested));
        let clean = self
            .root
            .confine_existing(&clean)
            .ok()
            .and_then(|clean| self.fs.metadata(&clean).ok().map(|meta| (clean, meta)));
        let Some((clean, meta)) =
            clean.filter(|(_, meta)| meta.kind == FileKind::File && !meta.is_special)
        else {
            return self.index_path_with_fingerprint(&placed.destination, None, None);
        };
        let own = self
            .fingerprints
            .fingerprint(&placed.destination, FileKind::File)?;
        let clean_fingerprint = self.fingerprints.fingerprint(&clean, meta.kind)?;
        if own != clean_fingerprint {
            return self.index_path_with_fingerprint(&placed.destination, Some(own), None);
        }
        // Empreintes égales : vérifier les octets avant de sacrifier la copie suffixée.
        if matches!(
            self.resolve_duplicate(
                &placed.destination,
                &clean,
                self.fingerprint_covers_file(meta.size)
            )?,
            DuplicateResolution::Distinct(_)
        ) {
            return self.index_path_with_fingerprint(&placed.destination, Some(own), None);
        }
        // Contenu identique au nom propre : cette copie n'est qu'un doublon, le nom propre gagne.
        let existing = match self.index_path_with_fingerprint(
            &clean,
            Some(clean_fingerprint),
            Some(&placed.requested),
        )? {
            LibraryOutcome::Indexed(record) | LibraryOutcome::Duplicate { existing: record } => {
                record
            }
        };
        if existing.path == placed.destination {
            // Ligne déjà propriétaire de cette copie : rien à dédupliquer.
            return Ok(LibraryOutcome::Indexed(existing));
        }
        Ok(LibraryOutcome::Duplicate { existing })
    }

    /// Destination shardée écrite sur disque, avant tout enregistrement en base.
    fn place_upload(
        &self,
        temp: &Path,
        original_name: &str,
        mtime: Option<OffsetDateTime>,
    ) -> Result<PlacedUpload, AppError> {
        let name = sanitize_filename(original_name)?;
        let files = self.files_path()?;
        if !self
            .fs
            .metadata(&files)
            .is_ok_and(|meta| meta.kind == FileKind::Directory)
        {
            return Err(AppError::Fs(FsError::Io(format!(
                "files unavailable: {}",
                display_lossy(&files)
            ))));
        }
        let mut state = PlacementState::default();
        let (destination, _, kind) =
            self.claim_destination(&files, &name, &mut state, |candidate| {
                self.link_or_copy(temp, candidate)
            })?;
        // La destination revendiquée est valide, le temporaire sera purgé au prochain démarrage.
        self.remove_quiet(temp, "upload temp cleanup failed");
        if let Err(error) = self.apply_owner_path(&destination) {
            tracing::warn!(path = %display_lossy(&destination), %error, "chown upload failed");
        }
        if let Some(mtime) = mtime
            && let Err(error) = self.fs.set_mtime(&destination, mtime)
        {
            tracing::warn!(path = %display_lossy(&destination), %error, "upload mtime restore failed");
        }
        Ok(PlacedUpload {
            destination,
            requested: name,
            kind,
        })
    }

    /// Revendique atomiquement une destination libre, suffixe `-<6 hex>` avec shard recalculé.
    pub(super) fn claim_destination<F>(
        &self,
        files: &Path,
        name: &str,
        state: &mut PlacementState,
        mut materialize: F,
    ) -> Result<(PathBuf, Placement, DestinationKind), AppError>
    where
        F: FnMut(&Path) -> Result<Placement, FsError>,
    {
        for attempt in 0..=COLLISION_RETRIES {
            let candidate_name = if attempt == 0 {
                name.to_string()
            } else {
                suffixed_name(name, &random_suffix()?)
            };
            let kind = if attempt == 0 {
                DestinationKind::Requested
            } else {
                DestinationKind::Suffixed
            };
            let candidate = sharded_path(files, OsStr::new(&candidate_name));
            let shard = candidate
                .parent()
                .ok_or_else(|| AppError::Invalid("sharded path has no parent".to_string()))?;
            // Canonicalisation et test d'écriture par shard, pas par fichier.
            let canonical_shard = self.canonical_shard(shard, files, state)?;
            let confined =
                canonical_shard.join(candidate.file_name().ok_or_else(|| {
                    AppError::Invalid("sharded path has no file name".to_string())
                })?);
            self.own_shard(&canonical_shard, state);
            match materialize(&confined) {
                Ok(placement) => return Ok((confined, placement, kind)),
                Err(FsError::AlreadyExists(_)) => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Err(AppError::Conflict(format!(
            "no free destination for {name} in {}",
            display_lossy(files)
        )))
    }

    /// Chemin canonique d'un shard et test d'écriture, mémorisés une fois par scan.
    fn canonical_shard(
        &self,
        shard: &Path,
        files: &Path,
        state: &mut PlacementState,
    ) -> Result<PathBuf, AppError> {
        if let Some(canonical) = state.shards.get(shard) {
            return Ok(canonical.clone());
        }
        self.create_shard(shard, state)?;
        let canonical = self.root.confine_existing(shard)?;
        if !self.fs.writable(&canonical) {
            return Err(AppError::Fs(FsError::Io(format!(
                "files not writable: {}",
                display_lossy(files)
            ))));
        }
        state.shards.insert(shard.to_path_buf(), canonical.clone());
        Ok(canonical)
    }

    /// Un `create_dir_all` par shard et par scan, pas un par fichier déposé.
    fn create_shard(&self, shard: &Path, state: &mut PlacementState) -> Result<(), AppError> {
        if state.created.contains(shard) {
            return Ok(());
        }
        self.fs.create_dir_all(shard)?;
        state.created.insert(shard.to_path_buf());
        Ok(())
    }

    /// Chown une seule fois par scan, `files/xx` puis `files/xx/yy` sinon root-owned après création.
    fn own_shard(&self, shard: &Path, state: &mut PlacementState) {
        let shard_parent = shard.parent();
        for dir in shard_parent.into_iter().chain(std::iter::once(shard)) {
            if !state.owned.insert(dir.to_path_buf()) {
                continue;
            }
            if let Err(error) = self.apply_owner_path(dir) {
                tracing::warn!(path = %display_lossy(dir), %error, "chown shard failed");
            }
        }
    }

    /// Lien matériel direct, repli copie exclusive quand les volumes diffèrent.
    pub(super) fn link_or_copy(
        &self,
        source: &Path,
        destination: &Path,
    ) -> Result<Placement, FsError> {
        match self.fs.link_noreplace(source, destination) {
            Ok(()) => Ok(Placement::Linked),
            Err(FsError::CrossDevice(_)) => {
                self.fs.copy_exclusive(source, destination)?;
                Ok(Placement::Copied)
            }
            Err(error) => Err(error),
        }
    }

    /// Purge les restes d'upload de plus de 24 h, appelée au démarrage du serveur.
    pub fn purge_upload_temps(&self) -> Result<usize, AppError> {
        let dir = self.upload_tmp_dir();
        let entries = self.read_dir_or_empty(&dir)?;
        let now = OffsetDateTime::now_utc();
        let mut removed = 0;
        for entry in entries {
            let path = dir.join(&entry.name);
            match entry.kind {
                FileKind::File if is_temp_file_name(&entry.name) => {
                    if self.remove_stale_temp(&path, now)? {
                        removed += 1;
                    }
                }
                FileKind::Directory if entry.name.as_os_str() == OsStr::new(DAV_DIR) => {
                    removed += self.purge_dav_temps(&path, now)?;
                }
                // Liens, autres répertoires et entrées spéciales : jamais suivis ni supprimés.
                _ => {}
            }
        }
        Ok(removed)
    }

    /// Purge `dav/<hash-collection>/<hash-nom>`, seuls noms produits par `path::temp_path`.
    fn purge_dav_temps(&self, dir: &Path, now: OffsetDateTime) -> Result<usize, AppError> {
        let collections = self.read_dir_or_empty(dir)?;
        let mut removed = 0;
        for collection in collections {
            if collection.kind != FileKind::Directory || !is_hash_name(&collection.name) {
                continue;
            }
            let collection_dir = dir.join(&collection.name);
            for entry in self.fs.read_dir(&collection_dir)? {
                if entry.kind != FileKind::File || !is_hash_name(&entry.name) {
                    continue;
                }
                let path = collection_dir.join(&entry.name);
                if self.remove_stale_temp(&path, now)? {
                    removed += 1;
                }
            }
        }
        Ok(removed)
    }

    fn remove_stale_temp(&self, path: &Path, now: OffsetDateTime) -> Result<bool, AppError> {
        let meta = self.fs.metadata(path)?;
        if now - meta.mtime <= UPLOAD_TEMP_MAX_AGE {
            return Ok(false);
        }
        self.fs.remove_file(path)?;
        Ok(true)
    }

    /// Contenu d'un dossier, vide si absent : purge silencieuse d'un temporaire jamais créé.
    fn read_dir_or_empty(&self, dir: &Path) -> Result<Vec<fs_ez_fs::DirEntryInfo>, AppError> {
        match self.fs.read_dir(dir) {
            Ok(entries) => Ok(entries),
            Err(FsError::NotFound(_)) => Ok(Vec::new()),
            Err(error) => Err(error.into()),
        }
    }
}
