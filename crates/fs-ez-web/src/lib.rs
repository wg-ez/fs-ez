#![forbid(unsafe_code)]
//! Interface web axum + askama + htmx.

pub mod api;
pub mod assets;
pub mod auth;
pub mod backend;
pub mod config;
pub(crate) mod cookie_util;
pub mod ctx;
pub mod dav;
pub mod error;
pub mod events;
pub(crate) mod forms;
pub mod handlers;
pub(crate) mod htmx;
pub mod locales;
pub(crate) mod negotiation;
pub mod prefs;
pub mod query_params;
pub mod redirect;
pub mod router;
pub mod state;
pub mod views;

#[cfg(test)]
mod layout_test;
#[cfg(test)]
pub(crate) mod test_support;
