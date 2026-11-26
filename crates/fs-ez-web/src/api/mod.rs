//! API JSON versionnée sous `/api` : mêmes cas d'usage que la CLI, jamais de contenu.

pub mod auth;
pub mod dto;
pub mod errors;
pub mod events;
pub mod files;
pub mod groups;
pub mod inbox;
pub mod status;
pub mod tag_action;
pub mod tags;
pub mod tokens;
pub mod view_groups;
pub mod views;

#[cfg(test)]
mod tests;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{delete, get, patch, post};

use crate::backend::Backend;
use crate::state::AppState;

use self::errors::API_BODY_LIMIT;

pub fn routes<B: Backend>() -> Router<AppState<B>> {
    Router::new()
        .route("/api/files", get(files::list))
        .route("/api/files/{id}", get(files::get))
        .route("/api/files/{id}/tags", post(files::mutate_tags))
        .route("/api/files/{id}/tags/{tag}", delete(files::remove_tag))
        .route("/api/tags", get(tags::list).post(tags::create))
        .route("/api/tags/{name}", patch(tags::patch).delete(tags::delete))
        .route("/api/views", get(views::list).post(views::create))
        .route(
            "/api/views/{id}",
            get(views::get).patch(views::patch).delete(views::delete),
        )
        .route("/api/views/{id}/files", get(views::files))
        .route("/api/groups", get(groups::list).post(groups::create))
        .route(
            "/api/groups/{id}",
            patch(groups::patch).delete(groups::delete),
        )
        .route("/api/groups/{id}/tags", post(groups::mutate_tags))
        .route(
            "/api/view-groups",
            get(view_groups::list).post(view_groups::create),
        )
        .route(
            "/api/view-groups/{id}",
            patch(view_groups::patch).delete(view_groups::delete),
        )
        .route(
            "/api/view-groups/{id}/views",
            post(view_groups::mutate_views),
        )
        .route("/api/inbox/scan", post(inbox::scan))
        .route("/api/tokens", get(tokens::list).post(tokens::create))
        .route("/api/tokens/{id}", delete(tokens::delete))
        .route("/api/status", get(status::get))
        .route("/api/events", get(events::stream))
        .layer(DefaultBodyLimit::max(API_BODY_LIMIT))
}
