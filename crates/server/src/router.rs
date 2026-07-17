use std::path::PathBuf;

use axum::{Router, http::StatusCode, routing::get};
use tower_http::services::{ServeDir, ServeFile};

use crate::{health::health, state::AppState};

pub fn build_router(state: AppState, web_root: impl Into<PathBuf>) -> Router {
    let web_root = web_root.into();
    let web_service =
        ServeDir::new(&web_root).fallback(ServeFile::new(web_root.join("index.html")));

    Router::new()
        .nest("/api", build_api_router(state))
        .fallback_service(web_service)
}

fn build_api_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .fallback(api_not_found)
        .with_state(state)
}

async fn api_not_found() -> StatusCode {
    StatusCode::NOT_FOUND
}
