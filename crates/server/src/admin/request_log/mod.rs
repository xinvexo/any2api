mod dto;
mod handlers;

use super::error;
use crate::state::AppState;
use axum::{Router, routing::get};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/request-logs", get(handlers::list))
        .route("/request-logs/{id}", get(handlers::get))
}
