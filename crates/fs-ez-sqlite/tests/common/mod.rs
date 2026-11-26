#![allow(dead_code)]

use std::path::Path;

use fs_ez_core::{FileId, FileKind, FilterExpr, Fingerprint, NewFile, Sort, TagColor, TagId};
use fs_ez_sqlite::convert::{dir_of, name_of};
use fs_ez_sqlite::{SqliteStore, db};
use fs_ez_store::{FileListQuery, FileStore, GroupStore, TagStore};
use time::OffsetDateTime;

pub fn store() -> SqliteStore {
    SqliteStore::from_connection(db::open_in_memory().expect("base memoire"))
}

pub fn new_file(
    path: &Path,
    fingerprint: Option<&str>,
    size: u64,
    mtime: OffsetDateTime,
) -> NewFile {
    NewFile {
        path_display: path.to_string_lossy().into_owned(),
        name: name_of(path),
        dir: dir_of(path),
        path: path.to_path_buf(),
        size,
        mtime,
        kind: FileKind::File,
        fingerprint: fingerprint.map(|raw| Fingerprint::new(raw).expect("empreinte")),
    }
}

pub fn insert(
    store: &SqliteStore,
    path: &str,
    fingerprint: Option<&str>,
    size: u64,
    mtime: OffsetDateTime,
) -> FileId {
    store
        .insert_file(&new_file(Path::new(path), fingerprint, size, mtime))
        .expect("insertion")
        .id
}

pub fn tag(store: &SqliteStore, name: &str) -> TagId {
    let group = match store.list_groups().expect("groupes").into_iter().next() {
        Some(group) => group,
        None => store.create_group("Tests", None).expect("groupe"),
    };
    store
        .get_or_create_tag(group.id, name, TagColor::Neutral)
        .expect("tag")
        .id
}

pub fn list_query(
    filter: Option<FilterExpr>,
    sort: Sort,
    offset: usize,
    limit: usize,
) -> FileListQuery {
    FileListQuery {
        filter,
        untagged: false,
        tagged_since_days: None,
        modified_since_days: None,
        text: None,
        virtual_folder: None,
        sort,
        offset,
        limit,
    }
}
