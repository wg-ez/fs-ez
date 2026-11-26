//! Racine fs-ez : découverte, layout et confinement des chemins.

use std::fs::{self, DirBuilder};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Component, Path, PathBuf};

use crate::error::FsError;
use crate::path::{display_lossy, normalize_relative};
use crate::pinned::PinnedDir;

const STATE_DIR: &str = "config";
const DEFAULT_FILES: &str = "files";
/// Marqueur de racine lors de la remontée depuis le CWD.
const PROJECT_FILE: &str = "fs-ez.toml";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Root {
    /// Toujours canonique.
    path: PathBuf,
    /// `FS_EZ_CONFIG` ou `<root>/config`, non canonisé pour accepter un dossier absent.
    state_dir: PathBuf,
}

impl Root {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn discover(explicit: Option<&Path>) -> Result<Self, FsError> {
        Self::discover_with(explicit, None)
    }

    /// Variante interne : `config` explicite prioritaire sur `FS_EZ_CONFIG` puis `<root>/config`.
    pub fn discover_with(explicit: Option<&Path>, config: Option<&Path>) -> Result<Self, FsError> {
        let config = config.map(Path::to_path_buf).or_else(env_config);
        let cwd = std::env::current_dir().map_err(|e| FsError::Io(format!("current dir: {e}")))?;
        Self::resolve(explicit, config.as_deref(), &cwd)
    }

    fn resolve(
        explicit: Option<&Path>,
        config: Option<&Path>,
        cwd: &Path,
    ) -> Result<Self, FsError> {
        let path = if let Some(path) = explicit {
            Self::canonical_dir(path)?
        } else {
            let mut found = None;
            for dir in cwd.ancestors() {
                if dir.join(PROJECT_FILE).is_file() {
                    found = Some(Self::canonical_dir(dir)?);
                    break;
                }
            }
            // Fallback cwd : permet `fs-ez init` dans un dossier neuf.
            match found {
                Some(path) => path,
                None => Self::canonical_dir(cwd)?,
            }
        };
        let state_dir = Self::resolve_state_dir(&path, config)?;
        Ok(Self { path, state_dir })
    }

    fn resolve_state_dir(root: &Path, configured: Option<&Path>) -> Result<PathBuf, FsError> {
        match configured {
            Some(value) => resolve_under(root, value),
            None => Ok(root.join(STATE_DIR)),
        }
    }

    fn canonical_dir(candidate: &Path) -> Result<PathBuf, FsError> {
        let path = fs::canonicalize(candidate)
            .map_err(|e| FsError::Invalid(format!("{}: {e}", display_lossy(candidate))))?;
        if !path.is_dir() {
            return Err(FsError::Invalid(format!(
                "not a directory: {}",
                display_lossy(candidate)
            )));
        }
        Ok(path)
    }

    pub fn state_dir(&self) -> PathBuf {
        self.state_dir.clone()
    }

    pub fn default_db(&self) -> PathBuf {
        self.state_dir().join("fs-ez.db")
    }

    pub fn files(&self, configured: &str) -> Result<PathBuf, FsError> {
        let raw = configured.trim();
        if raw.is_empty() {
            Ok(self.path.join(DEFAULT_FILES))
        } else {
            resolve_under(&self.path, Path::new(raw))
        }
    }

    /// Crée le dossier d'état (0700) et le dossier files (0755) ; ne supprime jamais rien.
    pub fn init_layout(&self, files: &str) -> Result<(), FsError> {
        self.create_private_dir()?;
        self.create_public_dir(&self.files(files)?)?;
        Ok(())
    }

    /// Dossier public 0755 idempotent, hors layout `files`.
    pub fn create_public_dir(&self, path: &Path) -> Result<(), FsError> {
        if path.is_dir() {
            return Ok(());
        }
        let mut builder = DirBuilder::new();
        builder.recursive(true).mode(0o755);
        match builder.create(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
            Err(error) => Err(FsError::from_io(path, error)),
        }
    }

    fn create_private_dir(&self) -> Result<(), FsError> {
        self.pinned_state_dir().map(|_| ())
    }

    /// Dossier d'état épinglé : chaque composant du chemin est ouvert sans suivre de lien.
    pub fn pinned_state_dir(&self) -> Result<PinnedDir, FsError> {
        let dir = PinnedDir::open_or_create_absolute(&self.state_dir(), 0o700)?;
        dir.set_mode(0o700)?;
        Ok(dir)
    }

    /// Canonise puis exige que le chemin reste sous le root (liens résolus).
    pub fn confine_existing(&self, path: &Path) -> Result<PathBuf, FsError> {
        let canonical = fs::canonicalize(path).map_err(|e| FsError::from_io(path, e))?;
        self.confine(path, canonical)
    }

    /// Confine un chemin encore inexistant via la canonicalisation de son parent.
    pub fn confine_new(&self, path: &Path) -> Result<PathBuf, FsError> {
        if path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(FsError::OutsideRoot(display_lossy(path)));
        }
        let file_name = path.file_name().ok_or_else(|| {
            FsError::Invalid(format!("missing file name: {}", display_lossy(path)))
        })?;
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let canonical_parent = fs::canonicalize(parent).map_err(|e| FsError::from_io(parent, e))?;
        self.confine(path, canonical_parent.join(file_name))
    }

    /// Exige que `resolved`, dérivé de `original`, reste sous le root.
    fn confine(&self, original: &Path, resolved: PathBuf) -> Result<PathBuf, FsError> {
        if self.contains(&resolved) {
            Ok(resolved)
        } else {
            Err(FsError::OutsideRoot(display_lossy(original)))
        }
    }

    pub fn contains(&self, path: &Path) -> bool {
        // `Path::starts_with` compare les composants : `/data2` n'est pas dans `/data`.
        path.starts_with(&self.path)
    }

    pub fn relative(&self, path: &Path) -> Option<PathBuf> {
        path.strip_prefix(&self.path).ok().map(Path::to_path_buf)
    }
}

/// `FS_EZ_CONFIG` non vide : dossier d'état hors du root.
fn env_config() -> Option<PathBuf> {
    std::env::var_os("FS_EZ_CONFIG")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Chemin absolu conservé tel quel, relatif normalisé puis ancré sous `root`.
fn resolve_under(root: &Path, raw: &Path) -> Result<PathBuf, FsError> {
    if raw.is_absolute() {
        Ok(raw.to_path_buf())
    } else {
        Ok(root.join(normalize_relative(&raw.to_string_lossy())?))
    }
}

#[cfg(test)]
mod tests;
