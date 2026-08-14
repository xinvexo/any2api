mod account;
mod events;
mod import;
mod login;
mod proxy_selection;
mod quota;
mod refresh_diagnostic;

use crate::state::AppState;
use axum::Router;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .merge(account::routes())
        .merge(login::routes())
        .merge(import::routes())
        .merge(quota::routes())
        .merge(events::routes())
}
