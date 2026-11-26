//! Scan de la bibliothèque : empreintes par lot, écritures groupées et repli unitaire.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use fs_ez_core::{FileId, FileKind, FilePatch, Fingerprint, NewFile};

use super::dedup::DuplicateResolution;
use super::*;

/// Fichiers par lot de scan : une recherche groupée par chemin et par empreinte, une transaction d'écriture.
const SCAN_BATCH: usize = 500;

/// Fichier du lot prêt à indexer : chemins du parcours et canonique, métadonnées et empreinte.
struct ScanEntry {
    path: PathBuf,
    confined: PathBuf,
    before: fs_ez_fs::FileMeta,
    existing: Option<FileRecord>,
    fingerprint: Fingerprint,
    fingerprint_complete: bool,
}

/// Décision d'une entrée, appliquée après les écritures groupées du lot.
enum ScanPlan {
    Duplicate(FileRecord),
    /// Doublon d'une entrée du même lot, enregistrement relu après écriture.
    DuplicateOf(ScanTarget),
    Update {
        id: FileId,
        patch: FilePatch,
    },
    Insert(usize),
    Skipped(String),
}

/// Entrée du même lot déjà écrite, source de l'enregistrement pour un doublon intra-lot.
#[derive(Clone, Copy)]
enum ScanTarget {
    Updated(FileId),
    Inserted(usize),
}

/// Propriétaire d'une empreinte : ligne en base ou entrée du lot courant.
enum FpOwner {
    Record(FileRecord),
    Target(ScanTarget),
}

impl FpOwner {
    /// Plan d'une entrée dont l'empreinte appartient déjà à un autre propriétaire.
    fn plan(&self) -> ScanPlan {
        match self {
            Self::Record(record) => ScanPlan::Duplicate(record.clone()),
            Self::Target(target) => ScanPlan::DuplicateOf(*target),
        }
    }
}

/// Place d'une entrée du parcours dans le rapport, ordre du parcours conservé.
enum ScanSlot {
    /// Fichier inchangé, jamais rouvert.
    Silent,
    Skip(PathBuf, String),
    Entry(usize),
}

impl App {
    pub fn scan_files(&self) -> Result<ImportReport, AppError> {
        let dir = self.files_path()?;
        // Canonicalisé une fois : les chemins du parcours n'ont aucun lien intermédiaire suivi.
        let canonical_dir = self.fs.canonicalize(&dir).ok();
        let report = RefCell::new(ImportReport::default());
        let mut batch: Vec<(PathBuf, fs_ez_fs::FileMeta)> = Vec::with_capacity(SCAN_BATCH);
        self.walk_files(
            &dir,
            |path, meta| {
                batch.push((path.to_path_buf(), meta.clone()));
                if batch.len() >= SCAN_BATCH {
                    self.scan_batch(
                        &mut batch,
                        &mut report.borrow_mut(),
                        &dir,
                        canonical_dir.as_deref(),
                    )?;
                }
                Ok(())
            },
            |_, _, _| Ok(()),
            |path, error| {
                report
                    .borrow_mut()
                    .skipped
                    .push((path.to_path_buf(), error.to_string()));
            },
        )?;
        self.scan_batch(
            &mut batch,
            &mut report.borrow_mut(),
            &dir,
            canonical_dir.as_deref(),
        )?;
        Ok(report.into_inner())
    }

    /// Lot : chemins canoniques sans `canonicalize` par fichier, empreintes préchargées par `IN`, écritures transactionnelles puis repli unitaire.
    fn scan_batch(
        &self,
        batch: &mut Vec<(PathBuf, fs_ez_fs::FileMeta)>,
        report: &mut ImportReport,
        dir: &Path,
        canonical_dir: Option<&Path>,
    ) -> Result<(), AppError> {
        if batch.is_empty() {
            return Ok(());
        }
        let prepared: Vec<(PathBuf, fs_ez_fs::FileMeta, Result<PathBuf, AppError>)> = batch
            .drain(..)
            .map(|(path, meta)| {
                // Chemin canonique sans `canonicalize` par fichier : `dir` canonique plus le relatif du parcours.
                let confined = match canonical_dir {
                    Some(canonical_dir) if let Ok(relative) = path.strip_prefix(dir) => {
                        let confined = canonical_dir.join(relative);
                        if self.root.contains(&confined) {
                            Ok(confined)
                        } else {
                            Err(AppError::Fs(FsError::OutsideRoot(display_lossy(&path))))
                        }
                    }
                    _ => self.root.confine_existing(&path).map_err(AppError::from),
                };
                (path, meta, confined)
            })
            .collect();
        let mut lookup: Vec<PathBuf> = Vec::with_capacity(prepared.len());
        let mut seen: HashSet<PathBuf> = HashSet::with_capacity(prepared.len());
        for (path, _, confined) in &prepared {
            // Forme brute aussi : une ligne historique non canonique reste retrouvée sans canonicalize.
            if let Ok(confined) = confined
                && seen.insert(confined.clone())
            {
                lookup.push(confined.clone());
            }
            if seen.insert(path.clone()) {
                lookup.push(path.clone());
            }
        }
        let mut by_path: HashMap<PathBuf, FileRecord> = HashMap::with_capacity(lookup.len());
        for record in self.store.find_files_by_paths(&lookup)? {
            by_path.entry(record.path.clone()).or_insert(record);
        }

        let mut slots = Vec::with_capacity(prepared.len());
        let mut entries: Vec<ScanEntry> = Vec::new();
        for (path, before, confined) in prepared {
            let confined = match confined {
                Ok(confined) => confined,
                Err(error) => {
                    slots.push(ScanSlot::Skip(path, error.to_string()));
                    continue;
                }
            };
            let existing = by_path
                .get(&confined)
                .or_else(|| by_path.get(&path))
                .cloned();
            if existing
                .as_ref()
                .is_some_and(|record| scan_is_unchanged(record, &before))
            {
                slots.push(ScanSlot::Silent);
                continue;
            }
            let fingerprint = match self.fingerprints.fingerprint(&confined, before.kind) {
                Ok(fingerprint) => fingerprint,
                Err(error) => {
                    tracing::warn!(path = %display_lossy(&path), %error, "scan fingerprint failed");
                    slots.push(ScanSlot::Skip(path, error.to_string()));
                    continue;
                }
            };
            // Nouveau stat après hachage : une variation invalide l'empreinte sans jamais réindexer à tort.
            let after = match self.fs.metadata(&confined) {
                Ok(meta) => meta,
                Err(error) => {
                    tracing::warn!(path = %display_lossy(&path), %error, "scan stat after hash failed");
                    slots.push(ScanSlot::Skip(path, error.to_string()));
                    continue;
                }
            };
            if after.kind != FileKind::File || after.is_special || metadata_changed(&before, &after)
            {
                slots.push(ScanSlot::Skip(path, "changed during scan".to_string()));
                continue;
            }
            slots.push(ScanSlot::Entry(entries.len()));
            let fingerprint_complete = self.fingerprint_covers_file(before.size);
            entries.push(ScanEntry {
                path,
                confined,
                before,
                existing,
                fingerprint,
                fingerprint_complete,
            });
        }
        if entries.is_empty() {
            // Sans entrée, aucun slot `Entry` n'est possible : seuls les skips restent à rapporter.
            for slot in slots {
                let ScanSlot::Skip(path, message) = slot else {
                    continue;
                };
                report.skipped.push((path, message));
            }
            return Ok(());
        }

        let fps: Vec<Fingerprint> = entries
            .iter()
            .map(|entry| entry.fingerprint.clone())
            .collect();
        // Empreinte → (propriétaire, ligne ou entrée du lot) : première entrée gagnante, doublons intra-lot relus après écriture.
        let mut owners: HashMap<String, (PathBuf, FpOwner)> =
            match self.store.find_files_by_fingerprints(&fps) {
                Ok(records) => by_fingerprint(records)
                    .into_iter()
                    .map(|(fingerprint, record)| {
                        (fingerprint, (record.path.clone(), FpOwner::Record(record)))
                    })
                    .collect(),
                // Repli sûr : les contraintes uniques du lot détectent le doublon puis l'unité tranche.
                Err(error) => {
                    tracing::warn!(%error, "scan batch fingerprint lookup failed");
                    HashMap::new()
                }
            };

        let mut plans: Vec<ScanPlan> = Vec::with_capacity(entries.len());
        let mut inserts: Vec<NewFile> = Vec::new();
        for entry in &entries {
            let mut fingerprint = entry.fingerprint.clone();
            let mut key = fingerprint.as_str().to_string();
            let mut duplicate = None;
            if let Some((owner, source)) = owners.get(&key)
                && owner != &entry.confined
            {
                match self.resolve_duplicate(&entry.confined, owner, entry.fingerprint_complete) {
                    Ok(DuplicateResolution::Duplicate) => {
                        duplicate = Some(source.plan());
                    }
                    Ok(DuplicateResolution::Distinct(full)) => {
                        // Bords communs, milieu différent : l'empreinte intégrale sépare les deux lignes.
                        fingerprint = full;
                        key = fingerprint.as_str().to_string();
                        if let Some((owner, source)) = owners.get(&key)
                            && owner != &entry.confined
                        {
                            duplicate = Some(source.plan());
                        }
                    }
                    Err(error) => {
                        tracing::warn!(path = %display_lossy(&entry.path), %error, "scan duplicate check failed");
                        plans.push(ScanPlan::Skipped(error.to_string()));
                        continue;
                    }
                }
            }
            if let Some(plan) = duplicate {
                plans.push(plan);
                continue;
            }
            let new =
                crate::files::new_file_from_meta(&entry.confined, &entry.before, Some(fingerprint));
            let (target, plan) = match &entry.existing {
                Some(existing) => {
                    // Séquencement identique au fichier par fichier : l'ancienne empreinte de la ligne disparaît ici.
                    if let Some(old) = &existing.fingerprint
                        && owners
                            .get(old.as_str())
                            .is_some_and(|(owner, _)| owner == &entry.confined)
                    {
                        owners.remove(old.as_str());
                    }
                    let plan = ScanPlan::Update {
                        id: existing.id,
                        patch: crate::files::patch_from_new(&new),
                    };
                    (ScanTarget::Updated(existing.id), plan)
                }
                None => {
                    inserts.push(new);
                    let target = ScanTarget::Inserted(inserts.len() - 1);
                    (target, ScanPlan::Insert(inserts.len() - 1))
                }
            };
            owners.insert(key, (entry.confined.clone(), FpOwner::Target(target)));
            plans.push(plan);
        }

        let mut updated: HashMap<FileId, FileRecord> = HashMap::with_capacity(plans.len());
        let updates: Vec<(FileId, FilePatch)> = plans
            .iter()
            .filter_map(|plan| match plan {
                ScanPlan::Update { id, patch } => Some((*id, patch.clone())),
                _ => None,
            })
            .collect();
        if !updates.is_empty() {
            let ids: Vec<FileId> = updates.iter().map(|(id, _)| *id).collect();
            match self.store.update_files_batch(&updates) {
                Ok(_) => self.load_scan_records(&ids, &mut updated),
                Err(error) => {
                    tracing::warn!(%error, "scan batch update failed, unit fallback");
                    for (entry, plan) in entries.iter().zip(plans.iter_mut()) {
                        if matches!(plan, ScanPlan::Update { .. })
                            && let Some(record) = self.unit_scan_entry(entry, plan)
                        {
                            updated.insert(record.id, record);
                        }
                    }
                }
            }
        }

        let mut inserted: Vec<Option<FileRecord>> = vec![None; inserts.len()];
        if !inserts.is_empty() {
            match self.store.insert_files_batch(&inserts) {
                Ok(ids) => {
                    let mut records = HashMap::with_capacity(ids.len());
                    self.load_scan_records(&ids, &mut records);
                    for (index, id) in ids.iter().enumerate() {
                        inserted[index] = records.get(id).cloned();
                    }
                }
                Err(error) => {
                    // Rollback global : chaque entrée repasse par le chemin unitaire, doublon concurrent compris.
                    tracing::warn!(%error, "scan batch insert failed, unit fallback");
                    for (entry, plan) in entries.iter().zip(plans.iter_mut()) {
                        let ScanPlan::Insert(slot) = *plan else {
                            continue;
                        };
                        if let Some(record) = self.unit_scan_entry(entry, plan) {
                            inserted[slot] = Some(record);
                        }
                    }
                }
            }
        }

        // Émission dans l'ordre du parcours, une entrée par slot.
        for slot in slots {
            let (path, plan) = match slot {
                ScanSlot::Silent => continue,
                ScanSlot::Skip(path, message) => {
                    report.skipped.push((path, message));
                    continue;
                }
                ScanSlot::Entry(index) => (&entries[index].path, &plans[index]),
            };
            match plan {
                ScanPlan::Duplicate(record) => {
                    report.duplicates.push((path.clone(), record.clone()));
                }
                ScanPlan::DuplicateOf(target) => {
                    let record = match target {
                        ScanTarget::Updated(id) => updated.get(id).cloned(),
                        ScanTarget::Inserted(index) => inserted.get(*index).cloned().flatten(),
                    };
                    match record {
                        Some(record) => report.duplicates.push((path.clone(), record)),
                        None => report
                            .skipped
                            .push((path.clone(), "duplicate target unavailable".to_string())),
                    }
                }
                ScanPlan::Update { id, .. } => match updated.get(id) {
                    Some(record) => report.imported.push(record.clone()),
                    None => report
                        .skipped
                        .push((path.clone(), "updated file unavailable".to_string())),
                },
                ScanPlan::Insert(index) => match inserted.get(*index).cloned().flatten() {
                    Some(record) => report.imported.push(record),
                    None => report
                        .skipped
                        .push((path.clone(), "inserted file unavailable".to_string())),
                },
                ScanPlan::Skipped(message) => report.skipped.push((path.clone(), message.clone())),
            }
        }
        Ok(())
    }

    /// Repli unitaire du scan : réindexe l'entrée et remplace son plan en cas de doublon ou d'échec.
    fn unit_scan_entry(&self, entry: &ScanEntry, plan: &mut ScanPlan) -> Option<FileRecord> {
        match self.index_path_with_fingerprint(
            &entry.confined,
            Some(entry.fingerprint.clone()),
            None,
        ) {
            Ok(LibraryOutcome::Indexed(record)) => Some(record),
            Ok(LibraryOutcome::Duplicate { existing }) => {
                *plan = ScanPlan::Duplicate(existing);
                None
            }
            Err(error) => {
                *plan = ScanPlan::Skipped(error.to_string());
                None
            }
        }
    }

    /// Relecture groupée des lignes écrites, repli unitaire si la requête `IN` échoue.
    fn load_scan_records(&self, ids: &[FileId], out: &mut HashMap<FileId, FileRecord>) {
        match self.store.get_files_by_ids(ids) {
            Ok(records) => {
                for record in records {
                    out.insert(record.id, record);
                }
            }
            Err(error) => {
                tracing::warn!(%error, "scan batch readback failed, unit fallback");
                for id in ids {
                    match self.store.get_file(*id) {
                        Ok(Some(record)) => {
                            out.insert(record.id, record);
                        }
                        Ok(None) => {}
                        Err(error) => tracing::warn!(file = id.0, %error, "scan readback failed"),
                    }
                }
            }
        }
    }
}

/// Un scan ne rouvre pas un fichier connu inchangé, ni ne recalcule son empreinte.
fn scan_is_unchanged(existing: &FileRecord, meta: &fs_ez_fs::FileMeta) -> bool {
    existing.fingerprint.is_some()
        && existing.size == meta.size
        && existing.kind == meta.kind
        && existing.mtime.unix_timestamp_nanos() == meta.mtime.unix_timestamp_nanos()
}
