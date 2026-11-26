//! Vues sauvegardées : façade des handlers mutation et édition.

mod form;
mod mutations;

pub use mutations::{
    create_view, delete_view, duplicate_view, edit_view, rename_view, set_view_color, update_view,
};
