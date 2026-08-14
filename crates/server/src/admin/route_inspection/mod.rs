mod dto;
mod handlers;

use axum::{Router, routing::get};

use crate::state::AppState;

pub(super) fn routes() -> Router<AppState> {
    Router::new().route("/route-inspection", get(handlers::get))
}
