//! Tests d'integration de `GroupStore` et `ViewStore` sur base memoire.

use fs_ez_core::{FilterExpr, NewSavedView, SavedViewPatch, Sort, SortDir, SortField, TagColor};
use fs_ez_sqlite::{SqliteStore, db};
use fs_ez_store::{GroupStore, StoreError, TagStore, ViewGroupStore, ViewStore};

mod common;

use common::{store, tag};

#[path = "groups_views_inbox/groups.rs"]
mod groups;
#[path = "groups_views_inbox/views.rs"]
mod views;

fn tag_filter(name: &str) -> FilterExpr {
    FilterExpr::Tag {
        tag: name.to_string(),
        negated: false,
    }
}

fn new_view(name: &str, filter: FilterExpr) -> NewSavedView {
    NewSavedView {
        name: name.to_string(),
        filter,
        icon: None,
        color: None,
        position: 0,
        sort: Sort {
            field: SortField::Id,
            dir: SortDir::Asc,
        },
    }
}
