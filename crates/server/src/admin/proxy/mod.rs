mod dto;
mod error;
mod handlers;

use super::revision;
use crate::state::AppState;
use axum::{
    Router,
    routing::{get, patch, post, put},
};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/proxies", get(handlers::list).post(handlers::create))
        .route(
            "/proxies/{id}",
            patch(handlers::update).delete(handlers::delete),
        )
        .route("/proxies/{id}/set-global", post(handlers::set_global))
        .route(
            "/proxies/{id}/authentication",
            put(handlers::set_authentication).delete(handlers::clear_authentication),
        )
        .route("/proxies/{id}/test", post(handlers::test))
}
