//! Implementation de `GroupStore` sur SQLite.

use fs_ez_core::{GroupId, Tag, TagColor, TagGroup, TagId};
use fs_ez_store::{GroupStore, StoreResult};
use rusqlite::Row;

use crate::convert::{TAG_COLUMNS, tag_from_row};
use crate::group_common::{self, GroupSpec, GroupUpdate};
use crate::store::{SqliteStore, collect_rows, db};

static SPEC: GroupSpec = GroupSpec {
    table: "tag_group",
    member_table: "tag_group_member",
    member_column: "tag_id",
    missing_after_insert: "inserted group is missing",
    not_found: "group",
};

const GROUP_TAG_IDS_SQL: &str =
    "SELECT m.tag_id FROM tag_group_member m JOIN tag t ON t.id = m.tag_id
     WHERE m.group_id = ?1 ORDER BY m.position, t.name_norm";

fn validate_group_name(raw: &str) -> StoreResult<String> {
    group_common::validate_group_name(raw, "group name")
}

fn group_from_row(row: &Row<'_>) -> rusqlite::Result<TagGroup> {
    group_common::group_from_row(row, |id, name, color, position, collapsed| TagGroup {
        id: GroupId(id),
        name,
        color,
        position,
        collapsed,
    })
}

impl GroupStore for SqliteStore {
    fn list_groups(&self) -> StoreResult<Vec<TagGroup>> {
        self.with_conn(|conn| group_common::list_groups(conn, SPEC.table, group_from_row))
    }

    fn get_group(&self, id: GroupId) -> StoreResult<Option<TagGroup>> {
        self.with_conn(|conn| group_common::group_by_id(conn, SPEC.table, id.0, group_from_row))
    }

    fn create_group(&self, name: &str, color: Option<TagColor>) -> StoreResult<TagGroup> {
        let name = validate_group_name(name)?;
        self.with_conn_mut(|conn| {
            group_common::create_group(conn, &SPEC, name, color, group_from_row)
        })
    }

    fn update_group(
        &self,
        id: GroupId,
        name: Option<&str>,
        color: Option<Option<TagColor>>,
        position: Option<i64>,
        collapsed: Option<bool>,
    ) -> StoreResult<TagGroup> {
        let name = match name {
            Some(raw) => Some(validate_group_name(raw)?),
            None => None,
        };
        self.with_conn(|conn| {
            group_common::update_group(
                conn,
                &SPEC,
                id.0,
                GroupUpdate {
                    name,
                    color,
                    position,
                    collapsed,
                },
                group_from_row,
            )
        })
    }

    fn delete_group(&self, id: GroupId) -> StoreResult<bool> {
        self.with_conn_mut(|conn| {
            // Supprimer les tags du groupe, les appartenances et `file_tag` suivent par cascade.
            group_common::delete_group(
                conn,
                SPEC.table,
                "DELETE FROM tag WHERE id IN (
                   SELECT tag_id FROM tag_group_member WHERE group_id = ?1
                 )",
                id.0,
            )
        })
    }

    fn group_for_tag(&self, tag: TagId) -> StoreResult<Option<GroupId>> {
        self.with_conn(|conn| {
            group_common::group_for_member(conn, SPEC.member_table, SPEC.member_column, tag.0)
                .map(|found| found.map(GroupId))
        })
    }

    fn place_tag(&self, group: GroupId, tag: TagId, before: Option<TagId>) -> StoreResult<()> {
        self.with_conn_mut(|conn| {
            group_common::place_member(
                conn,
                SPEC.member_table,
                SPEC.member_column,
                GROUP_TAG_IDS_SQL,
                group.0,
                tag.0,
                before.map(|before| before.0),
            )
        })
    }

    fn reorder_groups(&self, ids: &[GroupId]) -> StoreResult<()> {
        self.with_conn_mut(|conn| {
            group_common::reorder_groups(conn, SPEC.table, ids.iter().map(|id| id.0))
        })
    }

    fn reorder_group_tags(&self, group: GroupId, ids: &[TagId]) -> StoreResult<()> {
        self.with_conn_mut(|conn| {
            group_common::reorder_members(
                conn,
                SPEC.member_table,
                SPEC.member_column,
                group.0,
                ids.iter().map(|id| id.0),
            )
        })
    }

    fn tags_in_group(&self, group: GroupId) -> StoreResult<Vec<Tag>> {
        self.with_conn(|conn| {
            let sql = format!(
                "SELECT {TAG_COLUMNS} FROM tag_group_member m JOIN tag t ON t.id = m.tag_id
                 WHERE m.group_id = ?1 ORDER BY m.position, t.name_norm"
            );
            let mut statement = db(conn.prepare(&sql))?;
            collect_rows(&mut statement, [group.0], |row| tag_from_row(row, 0))
        })
    }
}
