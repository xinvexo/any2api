mod dto;
mod error;
mod events;
mod handlers;

use axum::Router;

use crate::state::AppState;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .merge(handlers::routes())
        .merge(events::routes())
}
