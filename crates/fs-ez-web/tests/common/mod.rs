#![allow(dead_code)]
#![allow(unused_imports)]

//! Harnais partage des tests d'integration web : fixture, requetes et sessions.

pub use axum::Router;
pub use axum::body::Body;
pub use axum::http::{Method, Request, Response, StatusCode, header};
pub use fs_ez_app::App;
pub use fs_ez_fingerprint::Blake3Fingerprinter;
pub use fs_ez_fs::{Root, SystemFs};
pub use fs_ez_sqlite::{SqliteStore, db};
pub use fs_ez_store::Store;
pub use fs_ez_web::backend::AppBackend;
pub use fs_ez_web::config::WebConfig;
pub use fs_ez_web::locales::Locales;
pub use fs_ez_web::router::build_router;
pub use fs_ez_web::state::AppState;
pub use std::sync::Arc;
pub use tempfile::TempDir;
pub use tower::ServiceExt;

mod assertions;
mod client;
mod fixture;
mod seed;

pub use assertions::*;
pub use client::*;
pub use fixture::*;
pub use seed::*;

pub const USERNAME: &str = "user";
pub const PASSWORD: &str = "correct-horse-battery";
