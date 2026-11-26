//! Routeur : surface publique (santé, login) et reste protégé par session + CSRF.

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::middleware::{from_fn, from_fn_with_state};
use axum::routing::{get, post};

use crate::auth::guards::require_auth;
use crate::backend::Backend;
use crate::error::WebError;
use crate::handlers;
use crate::router::middleware::{csrf_guard, persist_preferences};
use crate::state::AppState;

/// Marge d'en-têtes au-dessus de `MAX_UPLOAD`.
const BODY_OVERHEAD: u64 = 1024 * 1024;

pub fn routes<B: Backend>(state: AppState<B>) -> Router<AppState<B>> {
    let upload_limit = usize::try_from(state.config.max_upload.saturating_add(BODY_OVERHEAD))
        .unwrap_or(usize::MAX);
    let public = Router::new()
        .route("/health", get(handlers::health))
        .route("/static/{*path}", get(handlers::static_file))
        .route(
            "/login",
            get(handlers::login_page).post(handlers::login_submit),
        )
        .route(
            "/setup",
            get(handlers::setup_page).post(handlers::setup_submit),
        )
        .layer(from_fn(csrf_guard));

    let protected = Router::new()
        .merge(crate::api::routes::<B>())
        .route("/", get(handlers::ui::index))
        .route("/virtual-folders", get(handlers::ui::virtual_folders))
        .route(
            "/virtual-folders/{id}",
            get(handlers::ui::virtual_folder_view),
        )
        .route(
            "/partials/virtual-folders",
            post(handlers::ui::create_virtual_folder),
        )
        .route(
            "/partials/virtual-folders/rows",
            get(handlers::ui::virtual_folder_rows),
        )
        .route(
            "/partials/virtual-folders/{id}/rename",
            post(handlers::ui::rename_virtual_folder),
        )
        .route(
            "/partials/virtual-folders/{id}/delete",
            post(handlers::ui::delete_virtual_folder),
        )
        .route(
            "/partials/virtual-folders/{id}/move",
            post(handlers::ui::move_folder),
        )
        .route(
            "/partials/virtual-folders/move-selection",
            post(handlers::ui::move_selection),
        )
        .route(
            "/partials/virtual-folders/delete-selection",
            post(handlers::ui::delete_selection),
        )
        .route(
            "/partials/virtual-folders/{id}/tags",
            post(handlers::ui::tag_folder),
        )
        .route(
            "/partials/files/move",
            post(handlers::ui::move_files_folder),
        )
        .route("/views", post(handlers::ui::create_view))
        .route(
            "/views/{id}",
            get(handlers::ui::view).post(handlers::ui::update_view),
        )
        .route("/partials/views/{id}/edit", get(handlers::ui::edit_view))
        .route("/views/{id}/rename", post(handlers::ui::rename_view))
        .route("/views/{id}/color", post(handlers::ui::set_view_color))
        .route("/views/{id}/delete", post(handlers::ui::delete_view))
        .route("/views/{id}/duplicate", post(handlers::ui::duplicate_view))
        .route(
            "/partials/view-groups",
            post(handlers::ui::create_view_group),
        )
        .route(
            "/partials/view-groups/reorder",
            post(handlers::ui::reorder_view_groups),
        )
        .route(
            "/partials/view-groups/{id}/rename",
            post(handlers::ui::rename_view_group),
        )
        .route(
            "/partials/view-groups/{id}/color",
            post(handlers::ui::set_view_group_color),
        )
        .route(
            "/partials/view-groups/{id}/delete",
            post(handlers::ui::delete_view_group),
        )
        .route("/partials/views/place", post(handlers::ui::place_view))
        .route("/partials/files", get(handlers::ui::files_partial))
        .route("/partials/files_page", get(handlers::ui::files_partial))
        .route("/partials/files/rows", get(handlers::ui::files_rows))
        .route("/partials/files/tags", post(handlers::ui::mutate_bulk))
        .route("/partials/files/delete", post(handlers::ui::delete_files))
        .route(
            "/partials/files/{id}/rename",
            post(handlers::ui::rename_file),
        )
        .route(
            "/partials/file/{id}/tags/remove",
            post(handlers::ui::remove_file_tag),
        )
        .route("/settings", get(handlers::ui::settings_page))
        .route(
            "/settings/preferences",
            post(handlers::ui::update_preferences),
        )
        .route("/settings/backup", post(handlers::ui::update_backup))
        .route("/settings/backup/run", post(handlers::ui::run_backup))
        .route("/settings/tokens", post(handlers::ui::create_token))
        .route(
            "/settings/tokens/{id}/delete",
            post(handlers::ui::revoke_token),
        )
        .route("/partials/inbox/rescan", post(handlers::ui::rescan_inbox))
        .route(
            "/partials/sidebar/toggle",
            post(handlers::ui::toggle_sidebar),
        )
        .route("/partials/tags", post(handlers::ui::create_tag))
        .route("/partials/tags/place", post(handlers::ui::place_tag))
        .route("/partials/tags/{id}/rename", post(handlers::ui::rename_tag))
        .route(
            "/partials/tags/{id}/color",
            post(handlers::ui::set_tag_color),
        )
        .route("/partials/tags/{id}/delete", post(handlers::ui::delete_tag))
        .route("/partials/sidebar", get(handlers::ui::sidebar_partial))
        .route(
            "/partials/search_filter",
            get(handlers::ui::search_filter_partial),
        )
        .route("/partials/groups", post(handlers::ui::create_group))
        .route(
            "/partials/groups/reorder",
            post(handlers::ui::reorder_groups),
        )
        .route(
            "/partials/groups/{id}/rename",
            post(handlers::ui::rename_group),
        )
        .route(
            "/partials/groups/{id}/color",
            post(handlers::ui::set_group_color),
        )
        .route(
            "/partials/groups/{id}/delete",
            post(handlers::ui::delete_group),
        )
        .route("/logout", post(handlers::logout))
        .route("/api/session", get(handlers::api_session))
        .fallback(fallback)
        .layer(from_fn(csrf_guard))
        .layer(from_fn_with_state(state.clone(), persist_preferences::<B>))
        .layer(from_fn_with_state(state.clone(), require_auth::<B>));

    let dav = crate::dav::routes::<B>()
        .layer(DefaultBodyLimit::max(upload_limit))
        .layer(from_fn_with_state(
            state.clone(),
            crate::dav::auth::basic_auth::<B>,
        ))
        .layer(from_fn(crate::dav::logging::request_log));

    protected.merge(public).merge(dav)
}

pub async fn fallback() -> WebError {
    WebError::NotFound("route not found".to_string())
}
