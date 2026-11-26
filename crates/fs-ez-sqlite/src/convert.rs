//! Conversions entre le domaine et les colonnes SQLite.

use std::path::{Path, PathBuf};

use fs_ez_core::datetime::{format_rfc3339_utc, parse_rfc3339};
use fs_ez_core::{AppliedTag, FileId, FileKind, FileRecord, Fingerprint, Tag, TagColor, TagId};
use fs_ez_fs::{dir_lossy, display_lossy, file_name_lossy, path_bytes, path_from_bytes};
use fs_ez_store::StoreError;
use rusqlite::Row;
use rusqlite::types::Type;
use time::OffsetDateTime;
use time::UtcOffset;

/// Colonnes de `file` dans l'ordre attendu par `file_from_row`.
pub(crate) const FILE_COLUMNS: &str =
    "id, path, path_display, name, dir, size, mtime, kind, fingerprint, created_at, updated_at";

/// Position stockée pour un index d'ordre, jamais de troncature silencieuse.
pub(crate) fn position_from_index(index: usize) -> Result<i64, StoreError> {
    i64::try_from(index).map_err(|_| StoreError::Invalid("too many items to reorder".to_string()))
}

/// Compteur `COUNT(*)` lu en `i64`, négatif impossible côté SQL mais refusé explicitement.
pub(crate) fn to_count(value: i64) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::Db(format!("negative count {value}")))
}

/// Nombre de lignes affectées, `u64` sans troncature sur les cibles 32 bits.
pub(crate) fn row_count(changed: usize) -> Result<u64, StoreError> {
    u64::try_from(changed).map_err(|_| StoreError::Db(format!("invalid row count {changed}")))
}

pub fn ts_to_str(value: OffsetDateTime) -> String {
    format_rfc3339_utc(value)
}

pub fn ts_from_str(raw: &str) -> Result<OffsetDateTime, StoreError> {
    parse_rfc3339(raw)
        .map(|value| value.to_offset(UtcOffset::UTC))
        .map_err(|error| StoreError::Db(format!("invalid timestamp {raw:?}: {error}")))
}

/// Colonne timestamp obligatoire, erreur de conversion rattachee a son index.
pub(crate) fn ts_column(row: &Row<'_>, index: usize) -> rusqlite::Result<OffsetDateTime> {
    let raw: String = row.get(index)?;
    ts_from_str(&raw).map_err(|error| conversion(index, Type::Text, error))
}

/// Colonne timestamp nullable, `None` conserve tel quel.
pub(crate) fn opt_ts_column(
    row: &Row<'_>,
    index: usize,
) -> rusqlite::Result<Option<OffsetDateTime>> {
    let raw: Option<String> = row.get(index)?;
    raw.map(|raw| ts_from_str(&raw))
        .transpose()
        .map_err(|error| conversion(index, Type::Text, error))
}

pub fn path_to_blob(path: &Path) -> Vec<u8> {
    path_bytes(path).to_vec()
}

pub fn path_from_blob(raw: Vec<u8>) -> PathBuf {
    path_from_bytes(raw)
}

pub fn path_display(path: &Path) -> String {
    display_lossy(path)
}

pub fn name_of(path: &Path) -> String {
    file_name_lossy(path).unwrap_or_else(|| display_lossy(path))
}

pub fn dir_of(path: &Path) -> String {
    dir_lossy(path).unwrap_or_default()
}

pub fn file_kind_to_str(kind: FileKind) -> &'static str {
    kind.as_str()
}

pub fn file_kind_from_str(raw: &str) -> Result<FileKind, StoreError> {
    FileKind::parse(raw).ok_or_else(|| StoreError::Db(format!("unknown file kind {raw:?}")))
}

pub fn tag_color_to_str(color: TagColor) -> &'static str {
    color.as_token()
}

pub fn tag_color_from_str(raw: &str) -> Result<TagColor, StoreError> {
    TagColor::parse(raw).ok_or_else(|| StoreError::Db(format!("unknown tag color {raw:?}")))
}

pub fn file_from_row(row: &Row<'_>) -> rusqlite::Result<FileRecord> {
    file_from_row_at(row, 0)
}

/// Variante pour les jointures ou la ligne commence par d'autres colonnes.
pub(crate) fn file_from_row_at(row: &Row<'_>, offset: usize) -> rusqlite::Result<FileRecord> {
    let id: i64 = row.get(offset)?;
    let path: Vec<u8> = row.get(offset + 1)?;
    let path_display: String = row.get(offset + 2)?;
    let name: String = row.get(offset + 3)?;
    let dir: String = row.get(offset + 4)?;
    let size: i64 = row.get(offset + 5)?;
    let kind: String = row.get(offset + 7)?;
    let fingerprint: Option<String> = row.get(offset + 8)?;
    Ok(FileRecord {
        id: FileId(id),
        path: path_from_blob(path),
        path_display,
        name,
        dir,
        size: u64::try_from(size).map_err(|_| {
            conversion(
                offset + 5,
                Type::Integer,
                StoreError::Db(format!("negative size {size}")),
            )
        })?,
        mtime: ts_column(row, offset + 6)?,
        kind: file_kind_from_str(&kind)
            .map_err(|error| conversion(offset + 7, Type::Text, error))?,
        fingerprint: match fingerprint {
            Some(raw) => Some(
                Fingerprint::new(raw)
                    .map_err(|error| conversion(offset + 8, Type::Text, StoreError::from(error)))?,
            ),
            None => None,
        },
        created_at: ts_column(row, offset + 9)?,
        updated_at: ts_column(row, offset + 10)?,
    })
}

/// Colonnes de `tag` dans l'ordre attendu par `tag_from_row`.
pub(crate) const TAG_COLUMNS: &str =
    "t.id, t.name, t.name_norm, t.color, t.color_custom, t.created_at";

pub(crate) fn tag_from_row(row: &Row<'_>, offset: usize) -> rusqlite::Result<Tag> {
    let id: i64 = row.get(offset)?;
    let name: String = row.get(offset + 1)?;
    let name_norm: String = row.get(offset + 2)?;
    let color: String = row.get(offset + 3)?;
    let color_custom: Option<String> = row.get(offset + 4)?;
    Ok(Tag {
        id: TagId(id),
        name,
        name_norm,
        color: tag_color_from_str(&color)
            .map_err(|error| conversion(offset + 3, Type::Text, error))?,
        color_custom,
        created_at: ts_column(row, offset + 5)?,
    })
}

pub(crate) fn applied_tag_from_row(row: &Row<'_>) -> rusqlite::Result<AppliedTag> {
    applied_tag_from_row_at(row, 0)
}

pub(crate) fn applied_tag_from_row_at(
    row: &Row<'_>,
    offset: usize,
) -> rusqlite::Result<AppliedTag> {
    let tag = tag_from_row(row, offset)?;
    let explicit: i64 = row.get(offset + 6)?;
    Ok(AppliedTag {
        tag,
        explicit: explicit != 0,
        created_at: ts_column(row, offset + 7)?,
    })
}

pub(crate) fn conversion(index: usize, kind: Type, error: StoreError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(index, kind, Box::new(error))
}

#[cfg(test)]
mod tests;
