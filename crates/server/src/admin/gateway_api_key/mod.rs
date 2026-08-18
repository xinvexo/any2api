mod dto;
mod handlers;

#[cfg(test)]
pub(super) use dto::export_bindings;

use super::{error, request_json, revision};
use crate::state::AppState;
use axum::{
    Router,
    routing::{get, patch, post},
};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/gateway-api-keys",
            get(handlers::list).post(handlers::create),
        )
        .route(
            "/gateway-api-keys/{id}",
            patch(handlers::update).delete(handlers::delete),
        )
        .route("/gateway-api-keys/{id}/rotate", post(handlers::rotate))
}
