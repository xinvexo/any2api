mod account;
mod import;
mod login;
mod quota;

use crate::state::AppState;
use axum::Router;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .merge(account::routes())
        .merge(login::routes())
        .merge(import::routes())
        .merge(quota::routes())
}
