mod dto;
mod handlers;

pub(super) use dto::OverviewResourcesResponse;
#[cfg(test)]
pub(super) use dto::export_bindings;

use axum::{Router, routing::get};

use crate::state::AppState;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/overview/usage", get(handlers::usage))
        .route("/overview/resources", get(handlers::resources))
}
