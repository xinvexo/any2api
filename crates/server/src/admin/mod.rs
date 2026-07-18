mod error;
mod loopback;
mod model_route_dto;
mod model_route_handlers;
mod no_store;
mod provider_credential_dto;
mod provider_credential_handlers;
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
            "/model-routes",
            get(model_route_handlers::list).post(model_route_handlers::create),
        )
        .route(
            "/model-routes/{id}",
            axum::routing::patch(model_route_handlers::update).delete(model_route_handlers::delete),
        )
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
        .route(
            "/provider-endpoints/{endpoint_id}/credentials",
            get(provider_credential_handlers::list).post(provider_credential_handlers::create),
        )
        .route(
            "/provider-credentials/{id}",
            axum::routing::patch(provider_credential_handlers::update)
                .delete(provider_credential_handlers::delete),
        )
        .route(
            "/provider-credentials/{id}/rotate-secret",
            axum::routing::post(provider_credential_handlers::rotate_secret),
        )
        .fallback(error::not_found)
        .route_layer(middleware::from_fn(loopback::require_loopback))
}
