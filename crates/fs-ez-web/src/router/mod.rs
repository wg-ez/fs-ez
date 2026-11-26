//! Construction du routeur et tests `oneshot` sans socket.

pub mod middleware;
mod routes;

use axum::Router;
use axum::middleware::from_fn;

use crate::backend::Backend;
use crate::router::middleware::security_headers;
use crate::state::AppState;

pub use routes::fallback;

pub fn build_router<B: Backend>(state: AppState<B>) -> Router {
    routes::routes::<B>(state.clone())
        .layer(from_fn(security_headers))
        .with_state(state)
}

#[cfg(test)]
mod tests;
