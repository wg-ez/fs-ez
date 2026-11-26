//! Authentification web : session par cookie, guard de routes et limitation de débit.

pub mod guards;
pub mod login;
pub mod session;

pub use guards::SessionUser;
pub use login::LoginLimiter;
pub use session::{COOKIE_NAME, build_cookie, clear_cookie, read_cookie};
