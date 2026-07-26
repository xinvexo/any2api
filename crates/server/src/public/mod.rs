mod auth;
mod body;
mod error;
mod handlers;
mod models;
mod response;

use axum::{
    Router,
    extract::DefaultBodyLimit,
    middleware,
    routing::{any, get, post},
};

use crate::state::AppState;

/// Upper bound for buffered public request bodies. Large-context requests from
/// Codex CLI / Claude Code routinely exceed axum's 2 MB default, so the public
/// ingress accepts up to 32 MiB before rejecting with a protocol-shaped 413.
const MAX_PUBLIC_REQUEST_BYTES: usize = 32 * 1024 * 1024;

pub(crate) fn routes(state: AppState) -> Router {
    protected(
        Router::new()
            .route("/", any(error::not_found))
            .route("/models", get(models::list_models))
            .route("/responses", post(handlers::responses))
            .route("/responses/compact", post(handlers::responses_compact))
            .route("/chat/completions", post(handlers::chat_completions))
            .route("/messages", post(handlers::messages))
            .route(
                "/messages/count_tokens",
                post(handlers::messages_count_tokens),
            )
            .fallback(error::not_found)
            .method_not_allowed_fallback(error::method_not_allowed)
            .layer(DefaultBodyLimit::max(MAX_PUBLIC_REQUEST_BYTES)),
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
        .with_state(state)
}
