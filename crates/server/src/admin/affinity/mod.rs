mod dto;
mod handlers;

use super::{error, no_store};
use crate::state::AppState;
use axum::{
    Router,
    routing::{delete, get},
};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/affinity", get(handlers::get).delete(handlers::clear_all))
        .route(
            "/affinity/credentials/{id}",
            delete(handlers::clear_credential),
        )
}
