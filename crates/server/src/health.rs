use any2api_updater::api::APPLICATION_VERSION;
use axum::{
    Json,
    http::{HeaderValue, header::CACHE_CONTROL},
    response::{IntoResponse, Response},
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct HealthResponse {
    status: &'static str,
    application_version: &'static str,
}

pub(crate) async fn health() -> Response {
    let mut response = Json(HealthResponse {
        status: "ok",
        application_version: APPLICATION_VERSION,
    })
    .into_response();
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
