//! Contexte de template commun : base de page et sidebar déjà peuplée.

mod base;
mod choices;
mod sidebar;

pub use base::{Base, SearchField};
pub use choices::Choice;
pub use sidebar::{
    CreationError, GroupNav, SidebarColor, SidebarData, TagNav, ViewCreationError,
    ViewGroupCreationError, ViewGroupNav, ViewNav,
};
