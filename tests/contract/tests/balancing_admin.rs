use std::net::SocketAddr;

use any2api_contract_tests::TestApplication;
use any2api_domain::{
    ConfigRevision, CredentialId, CredentialKind, ProtocolDialect, ProviderCredentialDraft,
    ProviderEndpointDraft, ProviderEndpointId, ProviderKind, ProxyAddress, ProxyDraft, ProxyKind,
    ProxyProfileId, RequestsPerMinute,
};
use any2api_runtime::api::{ProviderApiKeySecret, SelectAndReserveResult, select_and_try_reserve};
use axum::{
    Router,
    body::Body,
    extract::ConnectInfo,
    http::{
        Method, Request, StatusCode,
        header::{CACHE_CONTROL, CONTENT_TYPE, VARY},
    },
};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

#[tokio::test]
async fn balancing_admin_exposes_only_aggregate_runtime_and_queue_policy() {
    let fixture = TestApplication::new().await;
    let publisher = fixture.publisher();
    let proxy_id = ProxyProfileId::new();
    let proxy = publisher
        .create_proxy(
            ConfigRevision::INITIAL,
            proxy_id,
            ProxyDraft::new(
                "Disabled Proxy",
                ProxyKind::Http,
                ProxyAddress::new("proxy.example.com", 8080).expect("proxy address"),
                false,
            )
            .expect("proxy draft"),
        )
        .await
        .expect("proxy publish");
    let endpoint_id = ProviderEndpointId::new();
    let credential_id = CredentialId::new();
    let endpoint = publisher
        .create_provider_endpoint(
            proxy.revision(),
            endpoint_id,
            ProviderEndpointDraft::new(
                "Codex Primary",
                ProviderKind::Codex,
                "https://api.example.com",
                ProtocolDialect::OpenAiResponses,
                None,
                true,
            )
            .expect("endpoint draft"),
        )
        .await
        .expect("endpoint publish");
    let published = publisher
        .create_provider_credential(
            endpoint.revision(),
            credential_id,
            endpoint_id,
            ProviderCredentialDraft::new(
                "Primary Key",
                CredentialKind::ApiKey,
                proxy_id,
                Some(RequestsPerMinute::new(2).expect("valid RPM")),
                true,
            )
            .expect("credential draft"),
            ProviderApiKeySecret::new("sk-balancing-contract".to_owned()),
        )
        .await
        .expect("credential publish");
    let binding = published
        .credential_runtime(credential_id.into())
        .expect("credential runtime");
    let permit = match select_and_try_reserve(std::slice::from_ref(binding), 0) {
        SelectAndReserveResult::Reserved(permit) => permit,
        result => panic!("expected RPM reservation, got {result:?}"),
    };
    let app = fixture.router();

    let (status, headers, body) = request(app.clone(), "/api/admin/balancing").await;
    assert_eq!(status, StatusCode::OK);
    assert_admin_cache_headers(&headers);
    assert_eq!(body["config_revision"], 4);
    assert_eq!(body["process"]["active_requests"], 0);
    assert_eq!(body["process"]["background_tasks"], 0);
    assert_eq!(body["queue"]["waiting"], 0);
    assert_eq!(body["queue"]["max_waiting"], 128);
    assert_eq!(body["queue"]["timeout_secs"], 180);
    assert_eq!(body["queue"]["on_rate_limited"], "wait");
    assert_eq!(body["queue"]["fallback_on_rate_limit"], false);
    assert_eq!(body["public_requests_in_window"], 0);
    assert!(body.get("auxiliary").is_none());
    assert_eq!(body["totals"]["in_flight"], 1);
    assert_eq!(body["totals"]["credential_count"], 1);
    assert_eq!(body["totals"]["enabled_credential_count"], 0);
    assert_eq!(body["totals"]["limited_credential_count"], 1);
    assert_eq!(body["totals"]["rate_limited_credential_count"], 0);
    assert_eq!(body["totals"]["requests_in_window"], 1);
    assert_eq!(body["totals"]["fixed_waiters"], 0);
    assert_eq!(body["totals"]["selected"], 0);
    let provider = &body["providers"][0];
    assert_eq!(provider["provider_kind"], "codex");
    assert_eq!(provider["credential_count"], 1);
    assert_eq!(provider["enabled_credential_count"], 0);
    assert_eq!(provider["limited_credential_count"], 1);
    assert_eq!(provider["rate_limited_credential_count"], 0);
    assert_eq!(provider["in_flight"], 1);
    assert_eq!(provider["requests_in_window"], 1);
    assert_eq!(provider["fixed_waiters"], 0);
    assert_eq!(provider["selected"], 0);
    assert!(body.get("credentials").is_none());
    let serialized = body.to_string();
    assert!(!serialized.contains("Primary Key"));
    assert!(!serialized.contains("Disabled Proxy"));
    assert!(!serialized.contains(&credential_id.to_string()));

    let (status, headers, body) = send(
        app.clone(),
        Method::PATCH,
        "/api/admin/settings",
        Body::from("{"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_request");
    assert_eq!(body["error"]["message"], "request body must be valid JSON");
    assert_admin_cache_headers(&headers);

    let (status, headers, body) = send(
        app.clone(),
        Method::DELETE,
        "/api/admin/settings/logs.request.enabled",
        Body::empty(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        body["error"]["message"],
        "expected_revision query is required"
    );
    assert_admin_cache_headers(&headers);

    let (status, headers, body) = send(
        app.clone(),
        Method::DELETE,
        "/api/admin/gateway-api-keys/00000000-0000-0000-0000-000000000001",
        Body::empty(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        body["error"]["message"],
        "expected_revision and expected_config_version queries are required"
    );
    assert_admin_cache_headers(&headers);

    let (status, headers, body) = request(app, "/api/admin/not-a-route").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "admin_api_not_found");
    assert_admin_cache_headers(&headers);
    drop(permit);
}

async fn request(app: Router, uri: &str) -> (StatusCode, axum::http::HeaderMap, Value) {
    send(app, Method::GET, uri, Body::empty()).await
}

async fn send(
    app: Router,
    method: Method,
    uri: &str,
    body: Body,
) -> (StatusCode, axum::http::HeaderMap, Value) {
    let response = app
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 41000))))
                .header(CONTENT_TYPE, "application/json")
                .body(body)
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    (
        status,
        headers,
        serde_json::from_slice(&bytes).expect("json"),
    )
}

fn assert_admin_cache_headers(headers: &axum::http::HeaderMap) {
    assert_eq!(headers.get(CACHE_CONTROL).expect("no-store"), "no-store");
    assert_eq!(headers["x-content-type-options"], "nosniff");
    assert!(
        headers.get_all(VARY).iter().any(|value| value == "Cookie"),
        "Vary: Cookie is missing"
    );
}
