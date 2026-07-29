mod dto;
mod handlers;

use super::{error, revision};
use crate::state::AppState;
use axum::{
    Router,
    routing::{get, patch},
};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/settings",
            get(handlers::list).patch(handlers::update_batch),
        )
        .route(
            "/settings/{key}",
            patch(handlers::update).delete(handlers::reset),
        )
}
