mod body;
mod dto;
mod handlers;
mod middleware;
mod policy;
mod refresh;

use axum::{Router, routing::get};

use crate::state::AppState;

pub(crate) use middleware::{ExcludeFromHttpAccessLog, HttpRequestId, record};
pub(crate) use refresh::is_automatic_log_refresh;

pub(crate) fn routes() -> Router<AppState> {
    Router::new().route("/system-logs", get(handlers::list).delete(handlers::clear))
}
