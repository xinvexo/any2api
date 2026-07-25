mod credential;
mod endpoint;

use crate::state::AppState;
use axum::Router;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .merge(endpoint::routes())
        .merge(credential::routes())
}
