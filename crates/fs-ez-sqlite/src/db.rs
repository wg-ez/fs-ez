//! Ouverture durcie de la base, pragmas et migrations versionnees.

use std::fs::{self, OpenOptions};
use std::io;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

use fs_ez_fs::{FsError, PinnedDir};
use rusqlite::Connection;

use crate::SqliteError;

const MIGRATIONS: &[(i64, &str)] = &[
    (1, include_str!("migrations/001_schema.sql")),
    (
        2,
        include_str!("migrations/002_api_token_credential_generation.sql"),
    ),
];

/// Base ouverte avec son parent epingle : garder `parent` vivant fige `-wal`/`-shm`.
pub struct Opened {
    pub conn: Connection,
    pub parent: Option<PinnedDir>,
}

/// Ouvre ou cree la base, puis applique les migrations manquantes.
pub fn open(path: &Path) -> Result<Connection, SqliteError> {
    Ok(open_pinned(path)?.conn)
}

/// Ouvre la base depuis un parent epingle, chaque composant refuse un lien.
pub fn open_pinned(path: &Path) -> Result<Opened, SqliteError> {
    let (target, parent) = match path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
        Some(parent_path) => {
            // Parent 0700 epingle : aucun ancetre symlink ne peut rediriger la creation.
            let parent =
                PinnedDir::open_or_create_absolute(parent_path, 0o700).map_err(fs_error)?;
            parent.set_mode(0o700).map_err(fs_error)?;
            let name = path.file_name().ok_or_else(|| {
                SqliteError::Io(io::Error::other("database path has no file name"))
            })?;
            (parent.child(name), Some(parent))
        }
        None => (path.to_path_buf(), None),
    };
    precreate(&target)?;
    let conn = Connection::open(&target)?;
    finish(&conn)?;
    harden(&target);
    Ok(Opened { conn, parent })
}

fn fs_error(error: FsError) -> SqliteError {
    SqliteError::Io(io::Error::other(error.to_string()))
}

/// Base memoire pour les tests, memes pragmas et memes migrations.
pub fn open_in_memory() -> Result<Connection, SqliteError> {
    let conn = Connection::open_in_memory()?;
    finish(&conn)?;
    Ok(conn)
}

// Pragmas puis migrations, dans cet ordre pour toutes les ouvertures.
fn finish(conn: &Connection) -> Result<(), SqliteError> {
    configure(conn)?;
    migrate(conn)
}

/// Lit `PRAGMA user_version`, c'est-a-dire la version du schema applique.
pub fn schema_version(conn: &Connection) -> Result<i64, SqliteError> {
    Ok(conn.query_row("PRAGMA user_version", [], |row| row.get(0))?)
}

// Pre-creation en 0600 avec O_NOFOLLOW, un symlink a cet emplacement echoue au lieu d'ecrire a travers.
fn precreate(path: &Path) -> Result<(), SqliteError> {
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW);
    match options.open(path) {
        Ok(_) => Ok(()),
        Err(error) if error.raw_os_error() == Some(libc::ELOOP) => {
            Err(SqliteError::Io(io::Error::other(format!(
                "refusing symlink database path: {}",
                path.display()
            ))))
        }
        Err(error) => Err(SqliteError::Io(error)),
    }
}

fn configure(conn: &Connection) -> Result<(), SqliteError> {
    // En memoire WAL n'a pas de sens, SQLite conserve alors le mode `memory`.
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.busy_timeout(Duration::from_secs(5))?;
    Ok(())
}

fn migrate(conn: &Connection) -> Result<(), SqliteError> {
    let latest = MIGRATIONS.last().map_or(0, |(version, _)| *version);
    let current = schema_version(conn)?;
    if current > latest {
        return Err(SqliteError::SchemaVersion(current, latest));
    }
    if current == latest {
        return Ok(());
    }
    conn.execute_batch("BEGIN IMMEDIATE")?;
    match apply_migrations(conn, latest) {
        Ok(()) => {
            conn.execute_batch("COMMIT")?;
            Ok(())
        }
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

// BEGIN IMMEDIATE peut attendre un autre processus, relire user_version sous verrou evite de rejouer une migration.
fn apply_migrations(conn: &Connection, latest: i64) -> Result<(), SqliteError> {
    let current = schema_version(conn)?;
    if current > latest {
        return Err(SqliteError::SchemaVersion(current, latest));
    }
    for (version, sql) in MIGRATIONS.iter().filter(|(version, _)| *version > current) {
        conn.execute_batch(sql)
            .map_err(|error| SqliteError::Migration(*version, error.to_string()))?;
        conn.pragma_update(None, "user_version", *version)
            .map_err(|error| SqliteError::Migration(*version, error.to_string()))?;
    }
    Ok(())
}

// Re-durcissement 0600 sur descripteur O_NOFOLLOW, ignore si le fichier n'existe pas encore.
fn harden(path: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let mut name = path.as_os_str().to_os_string();
        name.push(suffix);
        let target = PathBuf::from(name);
        let Ok(file) = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&target)
        else {
            continue;
        };
        let _ = file.set_permissions(fs::Permissions::from_mode(0o600));
    }
}

#[cfg(test)]
mod tests;
