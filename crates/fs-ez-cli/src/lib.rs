#![forbid(unsafe_code)]
//! Parsing CLI maison et formatage humain/JSON.

pub mod command;
pub mod context;
pub mod error;
pub mod globals;
pub mod output;
pub mod parse;
pub mod render;
pub mod run;
pub mod usage;

pub use command::{ALIASES, Command, resolve_alias};
pub use error::CliError;
pub use globals::GlobalOptions;
pub use output::Output;
pub use parse::{Parsed, parse};
pub use usage::{USAGE, usage_for};

#[cfg(test)]
mod layout_test;
