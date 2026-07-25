mod dto;
mod handlers;

use super::super::{error, revision};
use crate::state::AppState;
use axum::{
    Router,
    routing::{get, patch},
};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/provider-endpoints",
            get(handlers::list).post(handlers::create),
        )
        .route(
            "/provider-endpoints/{id}",
            patch(handlers::update).delete(handlers::delete),
        )
}
