mod auth;
mod error;
mod handlers;
mod models;
mod request_id;
mod response;

use axum::{
    Router, middleware,
    routing::{any, get, post},
};

use crate::state::AppState;

pub(crate) fn routes(state: AppState) -> Router {
    protected(
        Router::new()
            .route("/", any(error::not_found))
            .route("/models", get(models::list_models))
            .route("/responses", post(handlers::responses))
            .route("/responses/compact", post(handlers::responses_compact))
            .route("/messages", post(handlers::messages))
            .route(
                "/messages/count_tokens",
                post(handlers::messages_count_tokens),
            )
            .fallback(error::not_found)
            .method_not_allowed_fallback(error::method_not_allowed),
        state,
    )
}

pub(crate) fn namespace_root(state: AppState) -> Router {
    protected(Router::new().route("/v1/", any(error::not_found)), state)
}

fn protected(router: Router<AppState>, state: AppState) -> Router {
    router
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_gateway_api_key,
        ))
        .layer(middleware::from_fn(request_id::assign))
        .with_state(state)
}
