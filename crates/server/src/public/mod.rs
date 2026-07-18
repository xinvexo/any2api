mod auth;
mod error;
mod handlers;

use axum::{
    Router, middleware,
    routing::{get, post},
};

use crate::state::AppState;

pub(crate) fn routes(state: AppState) -> Router {
    Router::new()
        .route("/models", get(handlers::not_implemented))
        .route("/responses", post(handlers::not_implemented))
        .route("/responses/compact", post(handlers::not_implemented))
        .route("/messages", post(handlers::not_implemented))
        .route("/messages/count_tokens", post(handlers::not_implemented))
        .fallback(error::not_found)
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_gateway_api_key,
        ))
        .with_state(state)
}
