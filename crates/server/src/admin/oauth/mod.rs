mod dto;
mod handlers;
mod import_dto;
mod import_error;
mod login_dto;
mod quota_dto;
mod quota_error;
mod runtime_error;

use super::{error, no_store, revision, upstream_usage};
use crate::state::AppState;
use axum::Router;

pub(super) fn routes() -> Router<AppState> {
    handlers::routes()
}
