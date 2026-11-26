//! Implémentation de `VirtualFolderStore` sur SQLite.

use std::collections::HashMap;

use fs_ez_core::{
    FileId, VirtualFolder, VirtualFolderId, VirtualFolderMoveError, validate_virtual_folder_move,
};
use fs_ez_store::{StoreError, StoreResult, VirtualFolderStore};
use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, params, params_from_iter};
use time::OffsetDateTime;

use crate::convert::{ts_from_str, ts_to_str};
use crate::store::{SqliteStore, collect_rows, db, query_optional, with_immediate};

const VIRTUAL_FOLDER_COLUMNS: &str = "id, parent_id, name, position, created_at, updated_at";

fn folder_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawFolder> {
    Ok(RawFolder {
        id: row.get(0)?,
        parent: row.get(1)?,
        name: row.get(2)?,
        position: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

struct RawFolder {
    id: i64,
    parent: Option<i64>,
    name: String,
    position: i64,
    created_at: String,
    updated_at: String,
}

fn decode(row: RawFolder) -> StoreResult<VirtualFolder> {
    Ok(VirtualFolder {
        id: VirtualFolderId(row.id),
        parent: row.parent.map(VirtualFolderId),
        name: row.name,
        position: row.position,
        created_at: ts_from_str(&row.created_at)?,
        updated_at: ts_from_str(&row.updated_at)?,
    })
}

fn folder_by_id(conn: &Connection, id: VirtualFolderId) -> StoreResult<Option<VirtualFolder>> {
    let sql = format!("SELECT {VIRTUAL_FOLDER_COLUMNS} FROM virtual_folder WHERE id = ?1");
    query_optional(conn, &sql, [id.0], folder_from_row)?
        .map(decode)
        .transpose()
}

fn folder_by_parent_and_name(
    conn: &Connection,
    parent: Option<VirtualFolderId>,
    name: &str,
) -> StoreResult<Option<VirtualFolder>> {
    let sql = format!(
        "SELECT {VIRTUAL_FOLDER_COLUMNS} FROM virtual_folder
         WHERE parent_id IS ?1 AND name = ?2"
    );
    query_optional(
        conn,
        &sql,
        params![parent.map(|parent| parent.0), name],
        folder_from_row,
    )?
    .map(decode)
    .transpose()
}

fn next_position(conn: &Connection, parent: Option<VirtualFolderId>) -> StoreResult<i64> {
    db(conn.query_row(
        "SELECT COALESCE(MAX(position) + 1, 0) FROM virtual_folder WHERE parent_id IS ?1",
        [parent.map(|parent| parent.0)],
        |row| row.get(0),
    ))
}

/// Parent cible obligatoire : absent de la base, le déplacement ou le rattachement est refusé.
fn ensure_folder(conn: &Connection, folder: Option<VirtualFolderId>) -> StoreResult<()> {
    match folder {
        Some(folder) if folder_by_id(conn, folder)?.is_none() => {
            Err(StoreError::NotFound("virtual folder"))
        }
        _ => Ok(()),
    }
}

/// Parents de tous les dossiers, base de la validation d'un déplacement.
fn folder_parents(
    conn: &Connection,
) -> StoreResult<HashMap<VirtualFolderId, Option<VirtualFolderId>>> {
    let sql = format!("SELECT {VIRTUAL_FOLDER_COLUMNS} FROM virtual_folder");
    let mut statement = db(conn.prepare(&sql))?;
    let rows = collect_rows(&mut statement, [], folder_from_row)?;
    drop(statement);
    let mut parents = HashMap::with_capacity(rows.len());
    for row in rows {
        let folder = decode(row)?;
        parents.insert(folder.id, folder.parent);
    }
    Ok(parents)
}

/// Traduit le refus de déplacement commun en erreur de persistance.
fn map_move_error(error: VirtualFolderMoveError) -> StoreError {
    match error {
        VirtualFolderMoveError::Invalid(message) => StoreError::Invalid(message.to_string()),
        VirtualFolderMoveError::Missing(_) => StoreError::NotFound("virtual folder"),
    }
}

/// Affecte chaque fichier au dossier cible, ou le détache pour `None`.
fn set_files_folder(
    transaction: &Connection,
    files: &[FileId],
    folder: Option<VirtualFolderId>,
) -> StoreResult<()> {
    match folder {
        Some(folder) => {
            for file in files {
                db(transaction.execute(
                    "INSERT INTO virtual_folder_file(file_id, folder_id) VALUES (?1, ?2)
                     ON CONFLICT(file_id) DO UPDATE SET folder_id = excluded.folder_id",
                    params![file.0, folder.0],
                ))?;
            }
        }
        None => {
            for file in files {
                db(transaction.execute(
                    "DELETE FROM virtual_folder_file WHERE file_id = ?1",
                    [file.0],
                ))?;
            }
        }
    }
    Ok(())
}

/// Cœur des déplacements : cible et dossiers validés, puis dossiers et fichiers écrits dans la transaction.
fn move_entries(
    transaction: &Connection,
    files: &[FileId],
    folders: &[VirtualFolderId],
    parent: Option<VirtualFolderId>,
) -> StoreResult<()> {
    ensure_folder(transaction, parent)?;
    if !folders.is_empty() {
        let parents = folder_parents(transaction)?;
        for id in folders {
            if !parents.contains_key(id) {
                return Err(StoreError::NotFound("virtual folder"));
            }
            validate_virtual_folder_move(&parents, *id, parent).map_err(map_move_error)?;
        }
    }
    let now = ts_to_str(OffsetDateTime::now_utc());
    for id in folders {
        let changed = db(transaction.execute(
            "UPDATE virtual_folder SET parent_id = ?1, updated_at = ?2 WHERE id = ?3",
            params![parent.map(|parent| parent.0), now, id.0],
        ))?;
        if changed == 0 {
            return Err(StoreError::NotFound("virtual folder"));
        }
    }
    set_files_folder(transaction, files, parent)
}

impl VirtualFolderStore for SqliteStore {
    fn list_virtual_folders(&self) -> StoreResult<Vec<VirtualFolder>> {
        self.with_conn(|conn| {
            let sql = format!(
                "SELECT {VIRTUAL_FOLDER_COLUMNS} FROM virtual_folder
                 ORDER BY COALESCE(parent_id, 0), position, id"
            );
            let mut statement = db(conn.prepare(&sql))?;
            let rows = collect_rows(&mut statement, [], folder_from_row)?;
            rows.into_iter().map(decode).collect()
        })
    }

    fn get_virtual_folder(&self, id: VirtualFolderId) -> StoreResult<Option<VirtualFolder>> {
        self.with_conn(|conn| folder_by_id(conn, id))
    }

    fn find_virtual_folder(
        &self,
        parent: Option<VirtualFolderId>,
        name: &str,
    ) -> StoreResult<Option<VirtualFolder>> {
        self.with_conn(|conn| folder_by_parent_and_name(conn, parent, name))
    }

    fn create_virtual_folder(
        &self,
        parent: Option<VirtualFolderId>,
        name: &str,
    ) -> StoreResult<VirtualFolder> {
        let now = ts_to_str(OffsetDateTime::now_utc());
        self.with_conn(|conn| {
            ensure_folder(conn, parent)?;
            let position = next_position(conn, parent)?;
            db(conn.execute(
                "INSERT INTO virtual_folder(parent_id, name, position, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?4)",
                params![parent.map(|parent| parent.0), name, position, now],
            ))?;
            folder_by_id(conn, VirtualFolderId(conn.last_insert_rowid()))?
                .ok_or_else(|| StoreError::Db("inserted virtual folder is missing".to_string()))
        })
    }

    fn rename_virtual_folder(&self, id: VirtualFolderId, name: &str) -> StoreResult<VirtualFolder> {
        self.with_conn(|conn| {
            let changed = db(conn.execute(
                "UPDATE virtual_folder SET name = ?1, updated_at = ?2 WHERE id = ?3",
                params![name, ts_to_str(OffsetDateTime::now_utc()), id.0],
            ))?;
            if changed == 0 {
                return Err(StoreError::NotFound("virtual folder"));
            }
            folder_by_id(conn, id)?.ok_or(StoreError::NotFound("virtual folder"))
        })
    }

    fn move_virtual_folder(
        &self,
        id: VirtualFolderId,
        parent: Option<VirtualFolderId>,
    ) -> StoreResult<VirtualFolder> {
        self.with_conn_mut(|conn| {
            with_immediate(conn, |transaction| {
                move_entries(transaction, &[], &[id], parent)?;
                folder_by_id(transaction, id)?.ok_or(StoreError::NotFound("virtual folder"))
            })
        })
    }

    fn delete_virtual_folder(&self, id: VirtualFolderId) -> StoreResult<bool> {
        self.with_conn(|conn| {
            let changed = db(conn.execute("DELETE FROM virtual_folder WHERE id = ?1", [id.0]))?;
            Ok(changed > 0)
        })
    }

    fn move_virtual_selection(
        &self,
        files: &[FileId],
        folders: &[VirtualFolderId],
        parent: Option<VirtualFolderId>,
    ) -> StoreResult<usize> {
        if files.is_empty() && folders.is_empty() {
            return Ok(0);
        }
        self.with_conn_mut(|conn| {
            with_immediate(conn, |transaction| {
                move_entries(transaction, files, folders, parent)?;
                Ok(files.len() + folders.len())
            })
        })
    }

    fn folder_of_file(&self, file: FileId) -> StoreResult<Option<VirtualFolderId>> {
        self.with_conn(|conn| {
            let value: Option<i64> = query_optional(
                conn,
                "SELECT folder_id FROM virtual_folder_file WHERE file_id = ?1",
                [file.0],
                |row| row.get(0),
            )?;
            Ok(value.map(VirtualFolderId))
        })
    }

    fn attach_files_to_folder(
        &self,
        files: &[FileId],
        folder: Option<VirtualFolderId>,
    ) -> StoreResult<usize> {
        if files.is_empty() {
            return Ok(0);
        }
        self.with_conn_mut(|conn| {
            with_immediate(conn, |transaction| {
                ensure_folder(transaction, folder)?;
                set_files_folder(transaction, files, folder)?;
                Ok(files.len())
            })
        })
    }

    fn files_in_virtual_folder(&self, folder: Option<VirtualFolderId>) -> StoreResult<Vec<FileId>> {
        self.with_conn(|conn| {
            let (sql, values): (&str, Vec<SqlValue>) = match folder {
                Some(folder) => (
                    "SELECT m.file_id FROM virtual_folder_file m
                     JOIN file f ON f.id = m.file_id
                     WHERE m.folder_id = ?1 ORDER BY f.id",
                    vec![SqlValue::Integer(folder.0)],
                ),
                None => (
                    "SELECT f.id FROM file f
                     WHERE NOT EXISTS (
                       SELECT 1 FROM virtual_folder_file m WHERE m.file_id = f.id
                     ) ORDER BY f.id",
                    Vec::new(),
                ),
            };
            let mut statement = db(conn.prepare(sql))?;
            let rows = collect_rows(&mut statement, params_from_iter(values.iter()), |row| {
                row.get::<_, i64>(0)
            })?;
            Ok(rows.into_iter().map(FileId).collect())
        })
    }
}
