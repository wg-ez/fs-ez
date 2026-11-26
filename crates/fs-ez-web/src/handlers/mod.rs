//! Handlers partagés par le routeur.

pub mod auth;
pub mod health;
pub mod setup;
pub mod ui;

pub use auth::{api_session, login_page, login_submit, logout};
pub use health::health;
pub use setup::{setup_page, setup_submit};
pub use ui::static_file;
