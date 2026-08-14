use std::net::SocketAddr;

use any2api_contract_tests::TestApplication;
use any2api_storage::api::ConfigurationRepository;
use axum::{
    Router,
    body::Body,
    extract::ConnectInfo,
    http::{Method, Request, StatusCode, header::CONTENT_TYPE},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

#[tokio::test]
async fn provider_endpoint_contract_exposes_registry_options_and_publishes_crud() {
    let fixture = TestApplication::new().await;
    let storage = fixture.storage();
    let (_directory, app, _) = fixture.into_router();
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));

    let (status, initial) = request_json(
        app.clone(),
        Method::GET,
        "/api/admin/provider-endpoints",
        None,
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let protocol_options = initial["protocol_options"]
        .as_array()
        .expect("protocol options");
    let responses = protocol_option(protocol_options, "codex", "openai_responses");
    let direct = upstream_option(responses, "openai_responses");
    assert_eq!(direct["fidelity"], "direct");
    assert!(direct["bridge"].is_null());
    assert_eq!(
        direct["operations"],
        json!(["responses", "responses_compact", "alpha_search"])
    );

    let translated = upstream_option(responses, "openai_chat_completions");
    assert_eq!(translated["fidelity"], "translated");
    assert_eq!(translated["operations"], json!(["responses"]));
    assert_eq!(
        translated["bridge"]["contract_id"],
        "openai-responses-to-chat-completions/v1"
    );
    assert!(
        translated["bridge"]["request_fields"]
            .as_array()
            .is_some_and(|fields| fields.iter().any(|field| {
                field["path"] == "client_metadata" && field["behavior"] == "validated_only"
            }))
    );
    assert_eq!(translated["bridge"]["tool_types"], json!(["function"]));

    let images = protocol_option(protocol_options, "codex", "openai_images");
    let images_bridge = upstream_option(images, "openai_chat_completions");
    assert_eq!(images_bridge["fidelity"], "translated");
    assert_eq!(images_bridge["operations"], json!(["images_generations"]));
    assert_eq!(
        images_bridge["bridge"]["contract_id"],
        "openai-images-to-chat-completions/v1"
    );

    let (status, created) = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/provider-endpoints",
        Some(json!({
            "expected_revision": 1,
            "name": "Private Claude",
            "provider_kind": "claude",
            "base_url": "http://127.0.0.1:8443/",
            "protocol_dialect": "anthropic_messages",
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(created["items"][0]["base_url"], "http://127.0.0.1:8443");
    let endpoint_id = created["items"][0]["id"].as_str().expect("endpoint id");

    let (status, updated) = request_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/admin/provider-endpoints/{endpoint_id}"),
        Some(json!({
            "expected_revision": 2,
            "expected_config_version": 1,
            "name": "Private Claude Updated",
            "provider_kind": "claude",
            "base_url": "http://127.0.0.1:8443",
            "protocol_dialect": "anthropic_messages",
            "enabled": false
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["items"][0]["config_version"], 2);

    let (status, deleted) = request_json(
        app,
        Method::DELETE,
        &format!("/api/admin/provider-endpoints/{endpoint_id}?expected_revision=3"),
        None,
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(deleted["items"].as_array().is_some_and(Vec::is_empty));
    let stored = storage.load_configuration().await.expect("configuration");
    assert_eq!(stored.revision().get(), 4);
    assert!(stored.provider_endpoints().endpoints().is_empty());
}

fn protocol_option<'a>(options: &'a [Value], provider: &str, accepted: &str) -> &'a Value {
    options
        .iter()
        .find(|option| {
            option["provider_kind"] == provider && option["accepted_protocol"] == accepted
        })
        .expect("registered provider protocol option")
}

fn upstream_option<'a>(option: &'a Value, protocol: &str) -> &'a Value {
    option["upstream_options"]
        .as_array()
        .and_then(|options| {
            options
                .iter()
                .find(|candidate| candidate["protocol"] == protocol)
        })
        .expect("registered upstream protocol option")
}

async fn request_json(
    app: Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
    remote: SocketAddr,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .extension(ConnectInfo(remote));
    let body = if let Some(value) = body {
        builder = builder.header(CONTENT_TYPE, "application/json");
        Body::from(serde_json::to_vec(&value).expect("request json"))
    } else {
        Body::empty()
    };
    let response = app
        .oneshot(builder.body(body).expect("request"))
        .await
        .expect("response");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("response body")
        .to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).expect("response json"),
    )
}
