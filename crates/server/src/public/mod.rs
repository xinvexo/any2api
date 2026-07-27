mod auth;
mod body;
mod error;
mod handlers;
mod models;
mod response;

use any2api_runtime::api::{
    IMAGES_EDIT_REQUEST_BODY_LIMIT_BYTES, STANDARD_PUBLIC_REQUEST_BODY_LIMIT_BYTES,
};
use axum::{
    Router,
    extract::DefaultBodyLimit,
    middleware,
    routing::{any, get, post},
};

use crate::state::AppState;

pub(crate) fn routes(state: AppState) -> Router {
    // Keep text and Images generation JSON at 32 MiB while the authenticated
    // edit route uses its dedicated media upload limit from Runtime.
    let standard = Router::new()
        .route("/", any(error::not_found))
        .route("/models", get(models::list_models))
        .route("/responses", post(handlers::responses))
        .route("/responses/compact", post(handlers::responses_compact))
        .route("/chat/completions", post(handlers::chat_completions))
        .route("/images/generations", post(handlers::images_generations))
        .route("/messages", post(handlers::messages))
        .route(
            "/messages/count_tokens",
            post(handlers::messages_count_tokens),
        )
        .layer(DefaultBodyLimit::max(
            STANDARD_PUBLIC_REQUEST_BODY_LIMIT_BYTES,
        ));
    let image_edits = Router::new()
        .route("/images/edits", post(handlers::images_edits))
        .layer(DefaultBodyLimit::max(IMAGES_EDIT_REQUEST_BODY_LIMIT_BYTES));
    protected(
        standard
            .merge(image_edits)
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
        .with_state(state)
}
