//! Tests d'intégration de l'adaptateur WebDAV `/dav`.

#[path = "dav/support.rs"]
mod support;

#[path = "dav/auth.rs"]
mod auth;
#[path = "dav/content_range.rs"]
mod content_range;
#[path = "dav/delete.rs"]
mod delete;
#[path = "dav/inbox.rs"]
mod inbox;
#[path = "dav/locks.rs"]
mod locks;
#[path = "dav/moves.rs"]
mod moves;
#[path = "dav/read.rs"]
mod read;
#[path = "dav/tags.rs"]
mod tags;
#[path = "dav/validators.rs"]
mod validators;
#[path = "dav/views.rs"]
mod views;
#[path = "dav/virtual_folders.rs"]
mod virtual_folders;
#[path = "dav/write.rs"]
mod write;

mod common;
