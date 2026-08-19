use std::sync::LazyLock;

use any2api_updater::api::APPLICATION_VERSION;
use axum::{
    Json,
    http::{HeaderValue, header::CACHE_CONTROL},
    response::{IntoResponse, Response},
};
use serde::Serialize;
use uuid::Uuid;

static INSTANCE_ID: LazyLock<Uuid> = LazyLock::new(Uuid::new_v4);

#[derive(Debug, Serialize)]
pub(crate) struct HealthResponse {
    status: &'static str,
    application_version: &'static str,
    instance_id: Uuid,
}

pub(crate) async fn health() -> Response {
    let mut response = Json(HealthResponse {
        status: "ok",
        application_version: APPLICATION_VERSION,
        instance_id: *INSTANCE_ID,
    })
    .into_response();
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

#[cfg(test)]
mod tests {
    use axum::http::{StatusCode, header::CACHE_CONTROL};
    use http_body_util::BodyExt;
    use uuid::Uuid;

    use super::health;

    #[tokio::test]
    async fn health_instance_id_is_a_stable_process_uuid_and_is_not_cached() {
        let first = health().await;
        assert_eq!(first.status(), StatusCode::OK);
        assert_eq!(first.headers()[CACHE_CONTROL], "no-store");
        let first = body(first).await;
        let second = body(health().await).await;

        let first_id = first["instance_id"].as_str().expect("instance ID string");
        Uuid::parse_str(first_id).expect("instance ID UUID");
        assert_eq!(second["instance_id"], first_id);
        assert_eq!(first["status"], "ok");
        assert!(first["application_version"].is_string());
    }

    async fn body(response: axum::response::Response) -> serde_json::Value {
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("health body")
            .to_bytes();
        serde_json::from_slice(&bytes).expect("health JSON")
    }
}
