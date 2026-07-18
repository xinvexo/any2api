mod error;
mod loopback;
mod provider_endpoint_dto;
mod provider_endpoint_handlers;
mod proxy_dto;
mod proxy_handlers;
mod revision;

use axum::{Router, middleware, routing::get};

use crate::state::AppState;

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/proxies",
            get(proxy_handlers::list).post(proxy_handlers::create),
        )
        .route(
            "/proxies/{id}",
            axum::routing::patch(proxy_handlers::update).delete(proxy_handlers::delete),
        )
        .route(
            "/proxies/{id}/set-global",
            axum::routing::post(proxy_handlers::set_global),
        )
        .route(
            "/provider-endpoints",
            get(provider_endpoint_handlers::list).post(provider_endpoint_handlers::create),
        )
        .route(
            "/provider-endpoints/{id}",
            axum::routing::patch(provider_endpoint_handlers::update)
                .delete(provider_endpoint_handlers::delete),
        )
        .fallback(error::not_found)
        .route_layer(middleware::from_fn(loopback::require_loopback))
}
