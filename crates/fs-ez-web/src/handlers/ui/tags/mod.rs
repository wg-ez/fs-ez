//! Mutations des tags et groupes ; chaque réponse porte la sidebar OOB.

mod create;
mod finish;
mod mutate;
mod sidebar;
mod validation;

pub use create::create_tag;
pub(crate) use finish::{
    MutationKind, finish, finish_group, finish_group_create, finish_tag, finish_tag_create,
    finish_view_group, finish_view_group_create, no_content, user_error_message,
};
pub use mutate::{delete_tag, place_tag, rename_tag, set_tag_color};
pub use sidebar::{search_filter_partial, sidebar_partial};
pub(crate) use validation::{clean_tag_name, parse_group_id};
