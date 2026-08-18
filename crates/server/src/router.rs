use axum::{
    Router,
    http::{HeaderMap, Method, StatusCode, Uri},
    middleware,
    routing::{any, get},
};
use std::time::Duration;
use tower_http::services::{ServeDir, ServeFile};

use crate::{
    admin, embedded_web, health::health, http_access_log, public, request_body_timeout,
    request_lifecycle, response_security_headers, state::AppState, web_assets::WebAssets,
    web_security_headers,
};

pub fn build_router(state: AppState, web_assets: impl Into<WebAssets>) -> Router {
    let lifecycle = state.runtime().lifecycle();
    let body_idle_timeout = Duration::from_secs(
        state
            .snapshots()
            .load()
            .settings()
            .network()
            .request_body_idle_timeout_secs(),
    );
    let public_root = public::namespace_root(state.clone());
    let router = Router::new()
        .route("/api/", any(api_not_found))
        .merge(public_root)
        .nest("/api", build_api_router(state.clone()))
        .nest("/v1", public::routes(state.clone()));
    let web_router = match web_assets.into() {
        WebAssets::External(web_root) => Router::new()
            .nest_service("/assets", ServeDir::new(web_root.join("assets")))
            .fallback_service(
                ServeDir::new(&web_root).fallback(ServeFile::new(web_root.join("index.html"))),
            )
            .layer(middleware::from_fn(web_security_headers::add)),
        WebAssets::Embedded(assets) => Router::new().fallback(
            move |method: Method, uri: Uri, headers: HeaderMap| async move {
                embedded_web::response(&method, &uri, &headers, assets)
            },
        ),
    };
    router
        .merge(web_router)
        .layer(middleware::from_fn_with_state(
            lifecycle,
            request_lifecycle::track,
        ))
        .layer(middleware::from_fn(response_security_headers::add_nosniff))
        .layer(middleware::from_fn(move |request, next| {
            request_body_timeout::apply(request, next, body_idle_timeout)
        }))
        .layer(middleware::from_fn_with_state(
            state,
            http_access_log::record,
        ))
}

fn build_api_router(state: AppState) -> Router {
    Router::new()
        .route("/", any(api_not_found))
        .route("/health", get(health))
        .nest("/admin", admin::routes(state.clone()))
        .fallback(api_not_found)
        .with_state(state)
}

async fn api_not_found() -> StatusCode {
    StatusCode::NOT_FOUND
}
