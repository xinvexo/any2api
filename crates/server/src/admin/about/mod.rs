mod dto;
mod error;
mod handlers;

use axum::{
    Router,
    routing::{get, post},
};

use crate::state::AppState;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/about", get(handlers::about))
        .route("/update/check", post(handlers::check))
        .route("/update/install", post(handlers::install))
        .route("/update/status", get(handlers::status))
}
