mod access;
mod affinity_dto;
mod affinity_handlers;
mod auth_cookie;
mod auth_dto;
mod auth_handlers;
mod auth_middleware;
mod error;
mod gateway_api_key_dto;
mod gateway_api_key_handlers;
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
mod request_log_dto;
mod request_log_handlers;
mod revision;
mod settings_dto;
mod settings_handlers;

use axum::{
    Router, middleware,
    routing::{get, post},
};

use crate::state::AppState;

pub(crate) fn routes(state: AppState) -> Router<AppState> {
    let auth = Router::new()
        .route("/auth/session", get(auth_handlers::session))
        .route("/auth/setup", post(auth_handlers::setup))
        .route("/auth/login", post(auth_handlers::login));
    let protected = protected_routes().route_layer(middleware::from_fn_with_state(
        state,
        auth_middleware::require_admin_session,
    ));
    Router::new()
        .merge(auth)
        .merge(protected)
        .fallback(error::not_found)
        .layer(middleware::from_fn(no_store::responses))
}

fn protected_routes() -> Router<AppState> {
    Router::new()
        .route("/auth/logout", post(auth_handlers::logout))
        .route(
            "/affinity",
            get(affinity_handlers::get).delete(affinity_handlers::clear_all),
        )
        .route(
            "/affinity/credentials/{id}",
            axum::routing::delete(affinity_handlers::clear_credential),
        )
        .route(
            "/gateway-api-keys",
            get(gateway_api_key_handlers::list).post(gateway_api_key_handlers::create),
        )
        .route(
            "/gateway-api-keys/{id}",
            axum::routing::patch(gateway_api_key_handlers::update),
        )
        .route(
            "/gateway-api-keys/{id}/rotate",
            axum::routing::post(gateway_api_key_handlers::rotate),
        )
        .route(
            "/gateway-api-keys/{id}/revoke",
            axum::routing::post(gateway_api_key_handlers::revoke),
        )
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
        .route("/request-logs", get(request_log_handlers::list))
        .route("/request-logs/{id}", get(request_log_handlers::get))
        .route("/settings", get(settings_handlers::list))
        .route(
            "/settings/{key}",
            axum::routing::patch(settings_handlers::update).delete(settings_handlers::reset),
        )
}
