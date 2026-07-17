mod error;
mod loopback;
mod proxy_dto;
mod proxy_handlers;

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
        .fallback(error::not_found)
        .route_layer(middleware::from_fn(loopback::require_loopback))
}
