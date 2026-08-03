mod dto;
mod handlers;

use crate::state::AppState;
use axum::{Router, routing::get};

pub(super) fn routes() -> Router<AppState> {
    Router::new().route("/balancing", get(handlers::get))
}
