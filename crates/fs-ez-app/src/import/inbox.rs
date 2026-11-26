//! Import des dépôts physiques de l'inbox, par lots, avec repli unitaire.

use std::cell::RefCell;
use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;

use fs_ez_core::{FileId, FileKind, Fingerprint};
use fs_ez_fs::{Fs, PinnedDir};

use super::dedup::FingerprintResolution;
use super::naming::is_hidden_or_temporary;
use super::placement::{Placement, PlacementState};
use super::*;
use crate::files::sharded_path;

const MAX_SCAN_DEPTH: usize = 16;

/// Fichiers par lot d'empreintes, de recherches groupées et d'insertion transactionnelle.
const INBOX_BATCH: usize = 256;

/// Racine canonique du dépôt : descripteur épinglé et identité (dev, ino) du scan.
struct InboxRoot {
    inbox: PathBuf,
    canonical: PathBuf,
    dir: PinnedDir,
    dev: u64,
    ino: u64,
}

impl InboxRoot {
    /// Canonicalise l'inbox puis épingle son descripteur, refuse un chemin qui n'est pas un dossier.
    fn discover(fs: &dyn Fs, inbox: &Path) -> Result<Self, AppError> {
        let canonical = fs.canonicalize(inbox)?;
        let dir = PinnedDir::open(&canonical)?;
        let (dev, ino) = dir.identity()?;
        Ok(Self {
            inbox: inbox.to_path_buf(),
            canonical,
            dir,
            dev,
            ino,
        })
    }

    /// Parent épinglé d'une entrée, remonte depuis la racine figée sans suivre aucun lien.
    fn pin_parent(&self, path: &Path) -> Result<(PinnedDir, OsString), FsError> {
        self.ensure_same_root()?;
        let relative = path
            .strip_prefix(&self.inbox)
            .map_err(|_| FsError::OutsideRoot(display_lossy(path)))?;
        let name = relative
            .file_name()
            .ok_or_else(|| FsError::Invalid(format!("missing name: {}", display_lossy(path))))?
            .to_os_string();
        let parent = relative.parent().unwrap_or_else(|| Path::new(""));
        let parent = self.dir.open_at(parent)?;
        Ok((parent, name))
    }

    /// Parent épinglé et fichier régulier ouvert sans lien final, jamais un chemin re-résolu.
    fn pin_source(&self, path: &Path) -> Result<(PinnedDir, OsString, std::fs::File), FsError> {
        let (parent, name) = self.pin_parent(path)?;
        let (file, _) = parent.open_child_file(&name)?;
        Ok((parent, name, file))
    }

    fn ensure_same_root(&self) -> Result<(), FsError> {
        let meta = std::fs::metadata(&self.canonical)
            .map_err(|error| FsError::Io(format!("{}: {error}", display_lossy(&self.canonical))))?;
        if meta.dev() == self.dev && meta.ino() == self.ino {
            Ok(())
        } else {
            Err(FsError::OutsideRoot(display_lossy(&self.canonical)))
        }
    }
}

/// Invariants stables d'un import inbox, partagés par tous les lots.
struct ImportContext<'a> {
    files: &'a Path,
    same_device: bool,
    inbox: InboxRoot,
}

/// Fichier en attente d'empreinte, avec son dossier virtuel d'origine.
struct BatchEntry {
    source: PathBuf,
    meta: fs_ez_fs::FileMeta,
    folder: Vec<String>,
}

struct InboxEntry {
    source: PathBuf,
    folder: Vec<String>,
    before: fs_ez_fs::FileMeta,
    fingerprint: Fingerprint,
    fingerprint_complete: bool,
}

struct PendingInsert {
    source: PathBuf,
    destination: PathBuf,
    new_file: fs_ez_core::NewFile,
    folder: Vec<String>,
}

impl App {
    /// Parcours récursif déterministe des fichiers réguliers, profondeur bornée par `MAX_SCAN_DEPTH`.
    pub(super) fn walk_files(
        &self,
        dir: &Path,
        mut on_file: impl FnMut(&Path, &fs_ez_fs::FileMeta) -> Result<(), AppError>,
        mut on_dir: impl FnMut(&Path, u64, u64) -> Result<(), AppError>,
        mut on_error: impl FnMut(&Path, &FsError),
    ) -> Result<(), AppError> {
        let mut stack = vec![(dir.to_path_buf(), 0usize, None)];
        while let Some((current, depth, identity)) = stack.pop() {
            let listed = match identity {
                Some((dev, ino)) => self.fs.read_dir_checked(&current, dev, ino),
                None => self.fs.read_dir(&current),
            };
            let mut entries = match listed {
                Ok(entries) => {
                    // Dossier rouvert sous son identité : le dossier virtuel correspond à un dossier réel.
                    if let Some((dev, ino)) = identity {
                        on_dir(&current, dev, ino)?;
                    }
                    entries
                }
                Err(error) if current.as_path() == dir => return Err(error.into()),
                // Sous-dossier disparu pendant le parcours : rien à signaler.
                Err(FsError::NotFound(_)) => continue,
                // Lien ou inode différent : jamais descendre dans la cible.
                Err(error) => {
                    tracing::warn!(path = %display_lossy(&current), %error, "walk directory skipped");
                    continue;
                }
            };
            entries.sort_by(|a, b| a.name.as_bytes().cmp(b.name.as_bytes()));
            let mut subdirs = Vec::new();
            for entry in entries {
                if is_hidden_or_temporary(&entry.name) {
                    continue;
                }
                let path = current.join(&entry.name);
                match entry.kind {
                    FileKind::Directory => {
                        if depth < MAX_SCAN_DEPTH {
                            subdirs.push((path, depth + 1, Some((entry.dev, entry.ino))));
                        }
                    }
                    FileKind::File => {
                        let meta = match self.fs.metadata(&path) {
                            Ok(meta) => meta,
                            Err(error) => {
                                on_error(&path, &error);
                                continue;
                            }
                        };
                        if meta.kind != FileKind::File || meta.is_special {
                            continue;
                        }
                        on_file(&path, &meta)?;
                    }
                    _ => {}
                }
            }
            // Ordre déterministe : le dernier empilé sort en premier.
            for subdir in subdirs.into_iter().rev() {
                stack.push(subdir);
            }
        }
        Ok(())
    }

    /// Importe les dépôts physiques de l'inbox directement dans la bibliothèque.
    pub fn scan_inbox(&self) -> Result<ImportCounts, AppError> {
        let dir = self.inbox_path()?;
        let files = self.files_path()?;
        let inbox = InboxRoot::discover(self.fs.as_ref(), &dir)?;
        // Détection unique : même volume = lien direct, aucun temporaire ni copie disque.
        let same_device = match (std::fs::metadata(&dir), std::fs::metadata(&files)) {
            (Ok(dir), Ok(files)) => dir.dev() == files.dev(),
            _ => false,
        };
        let ctx = ImportContext {
            files: &files,
            same_device,
            inbox,
        };
        let counts = RefCell::new(ImportCounts::default());
        let mut state = PlacementState::default();
        let prefix = self.inbox_virtual_prefix();
        let mut dirs: Vec<(PathBuf, u64, u64)> = Vec::new();
        let mut batch: Vec<BatchEntry> = Vec::with_capacity(INBOX_BATCH);
        let mut flush = |batch: &mut Vec<BatchEntry>| {
            self.import_inbox_batch(batch, &ctx, &mut state, &mut counts.borrow_mut());
        };
        self.walk_files(
            &dir,
            |path, meta| {
                batch.push(BatchEntry {
                    source: path.to_path_buf(),
                    meta: meta.clone(),
                    folder: path
                        .parent()
                        .map_or_else(|| prefix.clone(), |parent| virtual_folder(&prefix, &dir, parent)),
                });
                if batch.len() >= INBOX_BATCH {
                    flush(&mut batch);
                }
                Ok(())
            },
            |path, dev, ino| {
                // Arborescence complète : un dossier vide reste visible dans les dossiers virtuels.
                let folder = virtual_folder(&prefix, &dir, path);
                if let Err(error) = self.ensure_virtual_folder_path(&folder) {
                    tracing::warn!(path = %display_lossy(path), %error, "inbox virtual folder create failed");
                }
                dirs.push((path.to_path_buf(), dev, ino));
                Ok(())
            },
            |path, error| {
                tracing::warn!(path = %display_lossy(path), %error, "inbox scan stat failed");
                counts.borrow_mut().skipped += 1;
            },
        )?;
        flush(&mut batch);
        self.remove_empty_inbox_dirs(&dir, &ctx.inbox, dirs);
        Ok(counts.into_inner())
    }

    /// Répertoires sources vidés par l'import, profondeur décroissante, jamais ceux qui gardent un fichier ignoré.
    fn remove_empty_inbox_dirs(
        &self,
        inbox: &Path,
        root: &InboxRoot,
        mut dirs: Vec<(PathBuf, u64, u64)>,
    ) {
        dirs.sort_by_key(|(path, _, _)| std::cmp::Reverse(path.components().count()));
        for (dir, dev, ino) in dirs {
            if dir == inbox || !dir.starts_with(inbox) {
                continue;
            }
            // Parent épinglé depuis la racine : un dossier remplacé par un lien n'est jamais atteint.
            let (parent, name) = match root.pin_parent(&dir) {
                Ok(value) => value,
                Err(error) => {
                    tracing::warn!(path = %display_lossy(&dir), %error, "inbox empty dir skipped");
                    continue;
                }
            };
            // Identité du dossier vidé vérifiée sur le descripteur avant toute suppression.
            let child = match parent.open_child_dir(&name) {
                Ok(child) => child,
                Err(_) => continue,
            };
            if !matches!(child.identity(), Ok(identity) if identity == (dev, ino)) {
                continue;
            }
            match child.read_dir() {
                Ok(entries) if entries.is_empty() => {
                    if let Err(error) = parent.remove_dir(&name)
                        && !matches!(error, FsError::NotFound(_))
                    {
                        tracing::warn!(path = %display_lossy(&dir), %error, "inbox empty dir cleanup failed");
                    }
                }
                Ok(_) | Err(FsError::NotFound(_)) => {}
                Err(error) => {
                    tracing::warn!(path = %display_lossy(&dir), %error, "inbox empty dir check failed");
                }
            }
        }
    }

    /// Lot : recherche groupée, placements, une transaction d'insertion, repli unitaire.
    fn import_inbox_batch(
        &self,
        batch: &mut Vec<BatchEntry>,
        ctx: &ImportContext<'_>,
        state: &mut PlacementState,
        counts: &mut ImportCounts,
    ) {
        if batch.is_empty() {
            return;
        }
        let mut entries = self.fingerprint_inbox_batch(batch, &ctx.inbox, counts);
        if entries.is_empty() {
            return;
        }
        let Some((records, existing_paths)) =
            self.inbox_batch_lookups(&mut entries, ctx, state, counts)
        else {
            return;
        };
        let by_fingerprint = super::by_fingerprint(records);
        let mut pending: HashSet<String> = HashSet::new();
        let mut inserts: Vec<PendingInsert> = Vec::new();
        let mut pending_duplicates: Vec<(PathBuf, Vec<String>)> = Vec::new();
        let mut deferred: Vec<(PathBuf, Vec<String>)> = Vec::new();
        for InboxEntry {
            source,
            folder,
            before,
            fingerprint,
            fingerprint_complete,
        } in entries
        {
            if let Some(existing) = by_fingerprint.get(fingerprint.as_str())
                && existing.path != source
            {
                if fingerprint_complete {
                    counts.duplicates += 1;
                    self.remove_inbox_source(&ctx.inbox, &source);
                } else {
                    // Bords identiques : le repli unitaire compare les octets et tranche.
                    deferred.push((source, folder));
                }
                continue;
            }
            if pending.contains(fingerprint.as_str()) {
                // Empreinte complète : doublon certain, suppression après commit du lot.
                if fingerprint_complete {
                    pending_duplicates.push((source, folder));
                } else {
                    // Échantillon identique : comparaison intégrale après commit du premier.
                    deferred.push((source, folder));
                }
                continue;
            }
            let Some((destination, reindex)) = self.place_inbox_source(
                &source,
                &before,
                ctx,
                state,
                counts,
                "inbox stability check failed",
            ) else {
                continue;
            };
            if reindex {
                // Lien muté : l'empreinte précalculée ne couvre plus la destination.
                self.index_inbox_destination(
                    &ctx.inbox,
                    &source,
                    &destination,
                    None,
                    &folder,
                    counts,
                );
                continue;
            }
            if existing_paths.contains(&destination) {
                // Ligne déjà présente à cette destination : mise à jour unitaire.
                self.index_inbox_destination(
                    &ctx.inbox,
                    &source,
                    &destination,
                    Some(fingerprint.clone()),
                    &folder,
                    counts,
                );
                continue;
            }
            pending.insert(fingerprint.as_str().to_string());
            let new_file =
                crate::files::new_file_from_meta(&destination, &before, Some(fingerprint));
            inserts.push(PendingInsert {
                source,
                destination,
                new_file,
                folder,
            });
        }
        self.commit_inbox_inserts(inserts, pending_duplicates, deferred, ctx, state, counts);
    }

    /// Empreintes et contrôle de stabilité autour du hachage, sans re-stat du parcours.
    fn fingerprint_inbox_batch(
        &self,
        batch: &mut Vec<BatchEntry>,
        root: &InboxRoot,
        counts: &mut ImportCounts,
    ) -> Vec<InboxEntry> {
        let mut entries = Vec::with_capacity(batch.len());
        for BatchEntry {
            source,
            meta: before,
            folder,
        } in batch.drain(..)
        {
            let Some((fingerprint, fingerprint_complete)) =
                self.fingerprint_inbox(&source, &before, root, counts)
            else {
                continue;
            };
            entries.push(InboxEntry {
                source,
                folder,
                before,
                fingerprint,
                fingerprint_complete,
            });
        }
        entries
    }

    /// Empreinte et contrôle de stabilité autour du hachage, `None` si une étape échoue.
    fn fingerprint_inbox(
        &self,
        source: &Path,
        before: &fs_ez_fs::FileMeta,
        root: &InboxRoot,
        counts: &mut ImportCounts,
    ) -> Option<(Fingerprint, bool)> {
        // Parent épinglé + fichier ouvert sans lien : le hachage ne peut pas sortir de l'inbox.
        let (parent, name, _held) = match root.pin_source(source) {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(path = %display_lossy(source), %error, "inbox source outside root");
                counts.skipped += 1;
                return None;
            }
        };
        let pinned = parent.child(&name);
        let fingerprint = match self.fingerprints.fingerprint(&pinned, FileKind::File) {
            Ok(fingerprint) => fingerprint,
            Err(error) => {
                tracing::warn!(path = %display_lossy(source), %error, "inbox fingerprint failed");
                counts.skipped += 1;
                return None;
            }
        };
        let after_hash = match self.fs.metadata(&pinned) {
            Ok(meta) if meta.kind == FileKind::File => meta,
            Ok(_) => {
                // Entrée remplacée par un lien ou un type spécial : jamais indexée.
                counts.skipped += 1;
                return None;
            }
            Err(error) => {
                tracing::warn!(path = %display_lossy(source), %error, "inbox stat after hash failed");
                counts.skipped += 1;
                return None;
            }
        };
        // Stable autour du hachage et sous le seuil : l'empreinte couvre tout le fichier.
        let fingerprint_complete =
            !metadata_changed(before, &after_hash) && self.fingerprint_covers_file(after_hash.size);
        Some((fingerprint, fingerprint_complete))
    }

    /// Commit du lot ; en cas d'échec, destinations retirées et repli unitaire des sources.
    fn commit_inbox_inserts(
        &self,
        inserts: Vec<PendingInsert>,
        pending_duplicates: Vec<(PathBuf, Vec<String>)>,
        deferred: Vec<(PathBuf, Vec<String>)>,
        ctx: &ImportContext<'_>,
        state: &mut PlacementState,
        counts: &mut ImportCounts,
    ) {
        let mut fallback: Vec<(PathBuf, Vec<String>)> = Vec::new();
        if inserts.is_empty() {
            self.import_units(
                pending_duplicates.into_iter().chain(deferred),
                ctx,
                state,
                counts,
            );
            return;
        }
        let news: Vec<fs_ez_core::NewFile> = inserts
            .iter()
            .map(|insert| insert.new_file.clone())
            .collect();
        match self.store.insert_files_batch(&news) {
            Ok(ids) => {
                for (insert, id) in inserts.iter().zip(ids) {
                    // Chown seulement après commit, puis la source n'est plus utile.
                    if let Err(error) = self.apply_owner_path(&insert.destination) {
                        tracing::warn!(path = %display_lossy(&insert.destination), %error, "chown inbox failed");
                    }
                    self.attach_import_folder(id, &insert.folder);
                    self.remove_inbox_source(&ctx.inbox, &insert.source);
                }
                counts.imported += inserts.len();
                for (source, _) in pending_duplicates {
                    counts.duplicates += 1;
                    self.remove_inbox_source(&ctx.inbox, &source);
                }
                self.import_units(deferred, ctx, state, counts);
            }
            Err(error) => {
                tracing::warn!(%error, "inbox batch insert failed, unit fallback");
                for insert in inserts {
                    self.remove_quiet(&insert.destination, "inbox rollback failed");
                    fallback.push((insert.source, insert.folder));
                }
                fallback.extend(pending_duplicates);
                fallback.extend(deferred);
                self.import_units(fallback, ctx, state, counts);
            }
        }
    }

    /// Indexe la destination avec l'empreinte fournie puis retire la source si acquise.
    fn index_inbox_destination(
        &self,
        root: &InboxRoot,
        source: &Path,
        destination: &Path,
        fingerprint: Option<Fingerprint>,
        folder: &[String],
        counts: &mut ImportCounts,
    ) {
        let outcome = match fingerprint {
            Some(fingerprint) => {
                self.index_path_with_fingerprint(destination, Some(fingerprint), None)
            }
            None => self.index_path(destination),
        };
        match outcome {
            Ok(LibraryOutcome::Indexed(record)) => {
                // Chown après succès seulement : un lien partage l'inode, une source conservée ne doit pas changer d'owner.
                if let Err(error) = self.apply_owner_path(destination) {
                    tracing::warn!(path = %display_lossy(destination), %error, "chown inbox failed");
                }
                self.attach_import_folder(record.id, folder);
                counts.imported += 1;
                self.remove_inbox_source(root, source);
            }
            Ok(LibraryOutcome::Duplicate { .. }) => {
                // Contenu déjà confirmé par l'indexation : destination inutile, source acquise.
                self.remove_quiet(destination, "inbox duplicate cleanup failed");
                counts.duplicates += 1;
                self.remove_inbox_source(root, source);
            }
            Err(error) => {
                self.remove_quiet(destination, "inbox rollback failed");
                tracing::warn!(path = %display_lossy(source), %error, "inbox import failed");
                counts.skipped += 1;
            }
        }
    }

    /// Revendique la destination d'une source inbox, vérifie sa stabilité et restaure le mtime d'une copie.
    /// Retourne la destination et `true` si la source a muté après un lien, donc à réindexer.
    fn place_inbox_source(
        &self,
        source: &Path,
        before: &fs_ez_fs::FileMeta,
        ctx: &ImportContext<'_>,
        state: &mut PlacementState,
        counts: &mut ImportCounts,
        error_label: &str,
    ) -> Option<(PathBuf, bool)> {
        // Parent épinglé + fichier régulier ouvert : le lien ou la copie reste dans l'inbox.
        let (parent, child, _held) = match ctx.inbox.pin_source(source) {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(path = %display_lossy(source), %error, "inbox source outside root");
                counts.skipped += 1;
                return None;
            }
        };
        let pinned = parent.child(&child);
        let name = import_name(source);
        let placed = self.claim_destination(ctx.files, &name, state, |candidate| {
            if ctx.same_device {
                self.link_or_copy(&pinned, candidate)
            } else {
                self.fs
                    .copy_exclusive(&pinned, candidate)
                    .map(|()| Placement::Copied)
            }
        });
        let (destination, placement, _) = match placed {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(path = %display_lossy(source), %error, "inbox placement failed");
                counts.skipped += 1;
                return None;
            }
        };
        let mut check = |watched: &Path| match self.fs.metadata(watched) {
            // Un lien final substitué à la source produit un lien dans la bibliothèque : refusé.
            Ok(after) if after.kind == FileKind::File => Some(metadata_changed(before, &after)),
            Ok(_) => {
                self.remove_quiet(&destination, error_label);
                tracing::warn!(path = %display_lossy(watched), "inbox placement is not a regular file");
                counts.skipped += 1;
                None
            }
            Err(error) => {
                self.remove_quiet(&destination, error_label);
                tracing::warn!(path = %display_lossy(watched), %error, "inbox stability check failed");
                counts.skipped += 1;
                None
            }
        };
        let mut reindex = false;
        match placement {
            // Lien : dest et source partagent l'inode, comparer la dest couvre les mutations de la source.
            Placement::Linked => reindex = check(&destination)?,
            // Copie : si la source a bougé, la copie peut être déchirée et n'est jamais indexée.
            Placement::Copied => {
                if check(source)? {
                    self.remove_quiet(&destination, "inbox unstable cleanup failed");
                    tracing::warn!(path = %display_lossy(source), "inbox source changed during copy, source kept");
                    counts.skipped += 1;
                    return None;
                }
            }
        }
        if placement == Placement::Copied
            && let Err(error) = self.fs.set_mtime(&destination, before.mtime)
        {
            tracing::warn!(path = %display_lossy(&destination), %error, "inbox mtime restore failed");
        }
        Some((destination, reindex))
    }

    /// Rejoue des sources différées ou en repli, dossier d'origine conservé.
    fn import_units(
        &self,
        sources: impl IntoIterator<Item = (PathBuf, Vec<String>)>,
        ctx: &ImportContext<'_>,
        state: &mut PlacementState,
        counts: &mut ImportCounts,
    ) {
        for (source, folder) in sources {
            self.import_inbox_file(&source, ctx, &folder, state, counts);
        }
    }

    /// Recherches groupées du lot, `None` après repli unitaire des sources si l'une échoue.
    fn inbox_batch_lookups(
        &self,
        entries: &mut Vec<InboxEntry>,
        ctx: &ImportContext<'_>,
        state: &mut PlacementState,
        counts: &mut ImportCounts,
    ) -> Option<(Vec<FileRecord>, HashSet<PathBuf>)> {
        let fps: Vec<Fingerprint> = entries
            .iter()
            .map(|entry| entry.fingerprint.clone())
            .collect();
        let mut fallback =
            |entries: &mut Vec<InboxEntry>, label: &str, error: &dyn std::fmt::Display| {
                tracing::warn!(%error, "{label}");
                self.import_units(
                    entries.drain(..).map(|entry| (entry.source, entry.folder)),
                    ctx,
                    state,
                    counts,
                );
            };
        let records = match self.store.find_files_by_fingerprints(&fps) {
            Ok(records) => records,
            Err(error) => {
                fallback(entries, "inbox batch duplicate lookup failed", &error);
                return None;
            }
        };
        let candidates: Vec<PathBuf> = entries
            .iter()
            .map(|entry| inbox_destination(ctx.files, &entry.source))
            .collect();
        match self.store.find_files_by_paths(&candidates) {
            Ok(path_records) => {
                let existing_paths = path_records.into_iter().map(|record| record.path).collect();
                Some((records, existing_paths))
            }
            Err(error) => {
                fallback(entries, "inbox batch destination lookup failed", &error);
                None
            }
        }
    }

    /// Rattache un fichier importé à son dossier d'origine, échec journalisé sans annuler l'import.
    fn attach_import_folder(&self, id: FileId, folder: &[String]) {
        if let Err(error) = self.attach_file_to_virtual_path(id, folder) {
            tracing::warn!(file = id.0, %error, "inbox virtual folder attach failed");
        }
    }

    /// Unlink d'une source après ré-ouverture du parent épinglé : un lien final n'est jamais suivi.
    fn remove_inbox_source(&self, root: &InboxRoot, source: &Path) {
        match root.pin_parent(source) {
            Ok((parent, name)) => {
                // Type relu sous le descripteur : un lien final est conservé, jamais sa cible.
                match parent.entry_kind(&name) {
                    Ok(FileKind::File) => {
                        if let Err(error) = parent.remove_file(&name)
                            && !matches!(error, FsError::NotFound(_))
                        {
                            tracing::warn!(path = %display_lossy(source), %error, "inbox source cleanup failed");
                        }
                    }
                    Ok(FileKind::Directory) => {
                        tracing::warn!(path = %display_lossy(source), "inbox source kept");
                    }
                    Ok(_) => {
                        tracing::warn!(path = %display_lossy(source), "inbox source kept");
                    }
                    Err(FsError::NotFound(_)) => {}
                    Err(error) => {
                        tracing::warn!(path = %display_lossy(source), %error, "inbox source kept");
                    }
                }
            }
            Err(FsError::NotFound(_)) => {}
            Err(error) => {
                tracing::warn!(path = %display_lossy(source), %error, "inbox source kept");
            }
        }
    }

    /// Repli unitaire : retire la source seulement quand l'import ou le doublon est vérifié.
    fn import_inbox_file(
        &self,
        path: &Path,
        ctx: &ImportContext<'_>,
        folder: &[String],
        state: &mut PlacementState,
        counts: &mut ImportCounts,
    ) {
        // Métadonnées fraîches : la taille du parcours ne prouve jamais que l'empreinte est complète.
        let before = match self.fs.metadata(path) {
            Ok(meta) if meta.kind == FileKind::File && !meta.is_special => meta,
            Ok(_) => {
                counts.skipped += 1;
                return;
            }
            Err(error) => {
                tracing::warn!(path = %display_lossy(path), %error, "inbox stat failed");
                counts.skipped += 1;
                return;
            }
        };
        let Some((mut fingerprint, fingerprint_complete)) =
            self.fingerprint_inbox(path, &before, &ctx.inbox, counts)
        else {
            return;
        };
        match self.resolve_fingerprint(path, fingerprint.clone(), fingerprint_complete) {
            Ok(FingerprintResolution::Available(resolved)) => fingerprint = resolved,
            Ok(FingerprintResolution::Duplicate(_)) => {
                counts.duplicates += 1;
                self.remove_inbox_source(&ctx.inbox, path);
                return;
            }
            Err(error) => {
                tracing::warn!(path = %display_lossy(path), %error, "inbox duplicate check failed");
                counts.skipped += 1;
                return;
            }
        }
        let Some((destination, reindex)) = self.place_inbox_source(
            path,
            &before,
            ctx,
            state,
            counts,
            "inbox unstable cleanup failed",
        ) else {
            return;
        };
        let fingerprint = if reindex { None } else { Some(fingerprint) };
        self.index_inbox_destination(&ctx.inbox, path, &destination, fingerprint, folder, counts);
    }
}

/// Destination probable avant collision, pour la recherche groupée par chemin.
fn inbox_destination(files: &Path, source: &Path) -> PathBuf {
    let name = import_name(source);
    sharded_path(files, OsStr::new(&name))
}

/// Chemin virtuel : préfixe inbox puis composants relatifs de `path`.
fn virtual_folder(prefix: &[String], inbox: &Path, path: &Path) -> Vec<String> {
    let mut folder = prefix.to_vec();
    let depth = crate::virtual_folders::MAX_VIRTUAL_FOLDER_DEPTH.saturating_sub(prefix.len());
    folder.extend(relative_components(inbox, path, depth));
    folder
}

#[cfg(test)]
mod tests;
