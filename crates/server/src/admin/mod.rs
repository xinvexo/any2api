mod about;
mod affinity;
mod auth;
mod balancing;
mod config_publish_error;
mod error;
mod gateway_api_key;
mod no_store;
mod oauth;
mod overview;
mod provider;
mod proxy;
mod request_log;
mod request_usage;
mod revision;
mod settings;
mod upstream_usage;

use axum::{Router, middleware};

use crate::state::AppState;

pub(crate) use error::AdminApiError;

pub(crate) fn routes(state: AppState) -> Router<AppState> {
    let protected = protected_routes().route_layer(middleware::from_fn_with_state(
        state,
        auth::require_admin_session,
    ));
    Router::new()
        .merge(auth::public_routes())
        .merge(protected)
        .fallback(error::not_found)
        .layer(middleware::from_fn(no_store::responses))
}

fn protected_routes() -> Router<AppState> {
    Router::new()
        .merge(about::routes())
        .merge(auth::protected_routes())
        .merge(affinity::routes())
        .merge(balancing::routes())
        .merge(oauth::routes())
        .merge(overview::routes())
        .merge(gateway_api_key::routes())
        .merge(proxy::routes())
        .merge(provider::routes())
        .merge(request_log::routes())
        .merge(crate::http_access_log::routes())
        .merge(settings::routes())
}
