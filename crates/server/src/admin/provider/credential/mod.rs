mod dto;
mod error;
mod handlers;

use super::super::{no_store, upstream_usage};
use crate::state::AppState;
use axum::{
    Router,
    routing::{get, patch, post, put},
};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/provider-endpoints/{endpoint_id}/credentials",
            get(handlers::list).post(handlers::create),
        )
        .route(
            "/provider-credentials/{id}",
            patch(handlers::update).delete(handlers::delete),
        )
        .route(
            "/provider-credentials/{id}/rotate-secret",
            post(handlers::rotate_secret),
        )
        .route("/provider-credentials/{id}/test", post(handlers::test))
        .route(
            "/provider-credentials/{id}/models",
            put(handlers::set_models),
        )
}
