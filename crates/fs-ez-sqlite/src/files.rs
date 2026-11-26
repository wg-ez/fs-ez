//! Implementation de `FileStore` sur SQLite.

mod listing;
mod tags;
mod util;
mod write;

use std::path::{Path, PathBuf};

use fs_ez_core::{
    AppliedTag, FileId, FilePatch, FileRecord, FileWithTags, FilterExpr, Fingerprint, NewFile,
    TagId,
};
use fs_ez_store::{FileListQuery, FileStore, Page, StoreResult};
use time::OffsetDateTime;

use crate::store::{SqliteStore, db};

impl FileStore for SqliteStore {
    fn insert_file(&self, new: &NewFile) -> StoreResult<FileRecord> {
        write::insert_file(self, new)
    }

    fn update_file(&self, id: FileId, patch: &FilePatch) -> StoreResult<FileRecord> {
        write::update_file(self, id, patch)
    }

    fn update_file_path_if(
        &self,
        id: FileId,
        expected_path: &Path,
        patch: &FilePatch,
    ) -> StoreResult<bool> {
        write::update_file_path_if(self, id, expected_path, patch)
    }

    fn update_files_batch(&self, updates: &[(FileId, FilePatch)]) -> StoreResult<usize> {
        write::update_files_batch(self, updates)
    }

    fn get_file(&self, id: FileId) -> StoreResult<Option<FileRecord>> {
        write::get_file(self, id)
    }

    fn get_files_by_ids(&self, ids: &[FileId]) -> StoreResult<Vec<FileRecord>> {
        write::get_files_by_ids(self, ids)
    }

    fn find_file_by_path(&self, path: &Path) -> StoreResult<Option<FileRecord>> {
        write::find_file_by_path(self, path)
    }

    fn find_file_by_name(&self, name: &str) -> StoreResult<Option<FileRecord>> {
        write::find_file_by_name(self, name)
    }

    fn find_file_by_fingerprint(&self, fp: &Fingerprint) -> StoreResult<Option<FileRecord>> {
        write::find_file_by_fingerprint(self, fp)
    }

    fn find_files_by_fingerprints(&self, fps: &[Fingerprint]) -> StoreResult<Vec<FileRecord>> {
        write::find_files_by_fingerprints(self, fps)
    }

    fn find_files_by_paths(&self, paths: &[PathBuf]) -> StoreResult<Vec<FileRecord>> {
        write::find_files_by_paths(self, paths)
    }

    fn insert_files_batch(&self, files: &[NewFile]) -> StoreResult<Vec<FileId>> {
        write::insert_files_batch(self, files)
    }

    fn delete_file(&self, id: FileId) -> StoreResult<bool> {
        write::delete_file(self, id)
    }

    fn delete_files_batch(&self, ids: &[FileId]) -> StoreResult<usize> {
        write::delete_files_batch(self, ids)
    }

    fn list_files(&self, query: &FileListQuery) -> StoreResult<Page<FileWithTags>> {
        listing::list_files(self, query)
    }

    fn count_files_query(&self, query: &FileListQuery) -> StoreResult<u64> {
        listing::count_files_query(self, query)
    }

    fn tags_for_file(&self, id: FileId) -> StoreResult<Vec<AppliedTag>> {
        tags::tags_for_file(self, id)
    }

    fn apply_tag(
        &self,
        file: FileId,
        tag: TagId,
        explicit: bool,
        at: OffsetDateTime,
    ) -> StoreResult<bool> {
        write::apply_tag(self, file, tag, explicit, at)
    }

    fn remove_tag(&self, file: FileId, tag: TagId) -> StoreResult<bool> {
        write::remove_tag(self, file, tag)
    }

    fn add_tags_batch(
        &self,
        files: &[FileId],
        tags: &[TagId],
        explicit: bool,
        at: OffsetDateTime,
    ) -> StoreResult<usize> {
        write::add_tags_batch(self, files, tags, explicit, at)
    }

    fn remove_tags_batch(&self, files: &[FileId], tags: &[TagId]) -> StoreResult<usize> {
        write::remove_tags_batch(self, files, tags)
    }

    fn replace_tags_batch(
        &self,
        files: &[FileId],
        tags: &[TagId],
        at: OffsetDateTime,
    ) -> StoreResult<usize> {
        write::replace_tags_batch(self, files, tags, at)
    }

    fn all_files(&self) -> StoreResult<Vec<FileRecord>> {
        listing::all_files(self)
    }

    fn count_files(&self) -> StoreResult<u64> {
        listing::count_files(self)
    }

    fn count_untagged(&self) -> StoreResult<u64> {
        listing::count_untagged(self)
    }

    fn count_tagged_since(&self, days: i64) -> StoreResult<u64> {
        listing::count_tagged_since(self, days)
    }

    fn count_matching(&self, filter: &FilterExpr) -> StoreResult<u64> {
        listing::count_matching(self, filter)
    }

    fn file_matches_filter(&self, id: FileId, filter: &FilterExpr) -> StoreResult<bool> {
        listing::file_matches_filter(self, id, filter)
    }

    /// `PRAGMA data_version` ne bouge que sur commit d'une autre connexion, jamais sur une écriture locale.
    fn data_version(&self) -> StoreResult<i64> {
        self.with_conn(|conn| db(conn.query_row("PRAGMA data_version", [], |row| row.get(0))))
    }
}
