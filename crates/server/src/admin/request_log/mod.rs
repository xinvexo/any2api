mod active_dto;
mod attempt_dto;
mod dto;
#[cfg(test)]
pub(super) use dto::export_bindings;
mod filter_options;
mod handlers;
mod query;

use super::error;
use crate::state::AppState;
use axum::{Router, routing::get};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/request-logs", get(handlers::list))
        .route("/request-logs/{id}", get(handlers::get))
}
