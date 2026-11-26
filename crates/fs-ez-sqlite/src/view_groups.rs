//! Implementation de `ViewGroupStore` sur SQLite.

use fs_ez_core::{SavedView, TagColor, ViewGroup, ViewGroupId, ViewId};
use fs_ez_store::{StoreResult, ViewGroupStore};
use rusqlite::Row;

use crate::group_common::{self, GroupSpec, GroupUpdate};
use crate::store::{SqliteStore, collect_rows, db};
use crate::views::{RawView, raw_view_from_row, view_from_raw};

static SPEC: GroupSpec = GroupSpec {
    table: "view_group",
    member_table: "view_group_member",
    member_column: "view_id",
    missing_after_insert: "inserted view group is missing",
    not_found: "view group",
};

const GROUPED_VIEW_COLUMNS: &str =
    "v.id, v.name, v.filter_json, v.icon, v.color, v.position, v.sort, v.created_at, v.updated_at";
const GROUP_VIEW_IDS_SQL: &str =
    "SELECT m.view_id FROM view_group_member m JOIN saved_view v ON v.id = m.view_id
     WHERE m.group_id = ?1 ORDER BY m.position, v.id";

fn validate_group_name(raw: &str) -> StoreResult<String> {
    group_common::validate_group_name(raw, "view group name")
}

fn view_group_from_row(row: &Row<'_>) -> rusqlite::Result<ViewGroup> {
    group_common::group_from_row(row, |id, name, color, position, collapsed| ViewGroup {
        id: ViewGroupId(id),
        name,
        color,
        position,
        collapsed,
    })
}

impl ViewGroupStore for SqliteStore {
    fn list_view_groups(&self) -> StoreResult<Vec<ViewGroup>> {
        self.with_conn(|conn| group_common::list_groups(conn, SPEC.table, view_group_from_row))
    }

    fn get_view_group(&self, id: ViewGroupId) -> StoreResult<Option<ViewGroup>> {
        self.with_conn(|conn| {
            group_common::group_by_id(conn, SPEC.table, id.0, view_group_from_row)
        })
    }

    fn create_view_group(&self, name: &str, color: Option<TagColor>) -> StoreResult<ViewGroup> {
        let name = validate_group_name(name)?;
        self.with_conn_mut(|conn| {
            group_common::create_group(conn, &SPEC, name, color, view_group_from_row)
        })
    }

    fn update_view_group(
        &self,
        id: ViewGroupId,
        name: Option<&str>,
        color: Option<Option<TagColor>>,
        position: Option<i64>,
        collapsed: Option<bool>,
    ) -> StoreResult<ViewGroup> {
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
                view_group_from_row,
            )
        })
    }

    fn delete_view_group(&self, id: ViewGroupId) -> StoreResult<bool> {
        self.with_conn_mut(|conn| {
            // Supprimer les vues du groupe, les appartenances suivent par cascade.
            group_common::delete_group(
                conn,
                SPEC.table,
                "DELETE FROM saved_view WHERE id IN (
                   SELECT view_id FROM view_group_member WHERE group_id = ?1
                 )",
                id.0,
            )
        })
    }

    fn group_for_view(&self, view: ViewId) -> StoreResult<Option<ViewGroupId>> {
        self.with_conn(|conn| {
            group_common::group_for_member(conn, SPEC.member_table, SPEC.member_column, view.0)
                .map(|found| found.map(ViewGroupId))
        })
    }

    fn place_view(
        &self,
        group: ViewGroupId,
        view: ViewId,
        before: Option<ViewId>,
    ) -> StoreResult<()> {
        self.with_conn_mut(|conn| {
            group_common::place_member(
                conn,
                SPEC.member_table,
                SPEC.member_column,
                GROUP_VIEW_IDS_SQL,
                group.0,
                view.0,
                before.map(|before| before.0),
            )
        })
    }

    fn reorder_view_groups(&self, ids: &[ViewGroupId]) -> StoreResult<()> {
        self.with_conn_mut(|conn| {
            group_common::reorder_groups(conn, SPEC.table, ids.iter().map(|id| id.0))
        })
    }

    fn reorder_group_views(&self, group: ViewGroupId, ids: &[ViewId]) -> StoreResult<()> {
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

    fn views_in_group(&self, group: ViewGroupId) -> StoreResult<Vec<SavedView>> {
        self.with_conn(|conn| {
            let sql = format!(
                "SELECT {GROUPED_VIEW_COLUMNS} FROM view_group_member m JOIN saved_view v ON v.id = m.view_id
                 WHERE m.group_id = ?1 ORDER BY m.position, v.id"
            );
            let mut statement = db(conn.prepare(&sql))?;
            let raws: Vec<RawView> = collect_rows(&mut statement, [group.0], raw_view_from_row)?;
            raws.into_iter().map(view_from_raw).collect()
        })
    }
}
