mod body;
mod dto;
mod handlers;
mod middleware;
mod policy;

use axum::{Router, routing::get};

use crate::state::AppState;

pub(crate) use middleware::{ExcludeFromHttpAccessLog, HttpRequestId, record};

pub(crate) fn routes() -> Router<AppState> {
    Router::new().route("/system-logs", get(handlers::list).delete(handlers::clear))
}
