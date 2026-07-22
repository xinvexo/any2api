use axum::{
    Router,
    http::{Method, StatusCode, Uri},
    middleware,
    routing::{any, get},
};
use tower_http::services::{ServeDir, ServeFile};

use crate::{
    admin, embedded_web, health::health, public, request_lifecycle, state::AppState,
    web_assets::WebAssets,
};

pub fn build_router(state: AppState, web_assets: impl Into<WebAssets>) -> Router {
    let lifecycle = state.runtime().lifecycle();
    let public_root = public::namespace_root(state.clone());
    let router = Router::new()
        .route("/api/", any(api_not_found))
        .merge(public_root)
        .nest("/api", build_api_router(state.clone()))
        .nest("/v1", public::routes(state));
    let router = match web_assets.into() {
        WebAssets::External(web_root) => router
            .nest_service("/assets", ServeDir::new(web_root.join("assets")))
            .fallback_service(
                ServeDir::new(&web_root).fallback(ServeFile::new(web_root.join("index.html"))),
            ),
        WebAssets::Embedded(assets) => {
            router.fallback(move |method: Method, uri: Uri| async move {
                embedded_web::response(&method, &uri, assets)
            })
        }
    };
    router.layer(middleware::from_fn_with_state(
        lifecycle,
        request_lifecycle::track,
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
