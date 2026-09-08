use std::{
    net::SocketAddr,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use any2api_contract_tests::TestApplication;
use any2api_domain::{
    CompletedRequestLog, ProtocolDialect, ProtocolOperation, RequestId, RequestLog, SettingKey,
    SettingValue, TokenUsage,
};
use any2api_runtime::api::RequestTelemetry;
use any2api_storage::api::RequestLogRepository;
use axum::{body::Body, extract::ConnectInfo, http::Request};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

#[tokio::test]
async fn persisted_request_cost_keeps_its_exchange_rate_after_settings_change() {
    let app = TestApplication::new().await;
    let snapshot = app.snapshots().load();
    let card = snapshot.settings().oauth().codex_rate_card();
    let quota_cost = card
        .cost_rates("gpt-5.6-sol")
        .expect("model rates")
        .estimate(TokenUsage::new(Some(2_000), Some(100), Some(500)), None)
        .expect("complete usage");
    let request_id = RequestId::new();
    let started_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_millis() as u64;
    let record = CompletedRequestLog {
        request: RequestLog {
            request_id,
            started_at_ms,
            client_ip: "127.0.0.1".parse().expect("IP"),
            config_revision: snapshot.revision(),
            gateway_api_key_id: None,
            ingress_protocol: ProtocolDialect::OpenAiResponses,
            operation: ProtocolOperation::Responses,
            public_model: Some("gpt-5.6-sol".into()),
            thinking_level: None,
            provider_endpoint_id: None,
            credential_id: None,
            oauth_account_id: None,
            proxy_profile_id: None,
            status_code: 200,
            error_class: None,
            error_message: None,
            attempt_count: 0,
            latency_ms: 10,
            first_token_ms: None,
            input_tokens: Some(2_000),
            output_tokens: Some(100),
            cache_read_tokens: Some(500),
            cache_creation_tokens: None,
            quota_cost: Some(quota_cost),
            is_stream: false,
            requested_speed_tier: None,
            effective_speed_tier: None,
        },
        attempts: Vec::new(),
        telemetry_position: None,
    };
    app.storage()
        .append_request_logs(&[record], 100)
        .await
        .expect("record request");
    let mut next_card = card.clone();
    next_card.id = "updated-rate-card".into();
    next_card.credits_per_usd = 100;
    app.publisher()
        .set_setting_override(
            snapshot.revision(),
            SettingKey::OAuthCodexRateCard,
            SettingValue::CodexRateCard(next_card),
        )
        .await
        .expect("change rate card");

    let telemetry = Arc::new(RequestTelemetry::start(
        app.storage(),
        snapshot.revision(),
        snapshot.settings().logging(),
        &app.runtime().lifecycle(),
    ));
    let state = app.state().with_request_telemetry(Arc::clone(&telemetry));
    let (_directory, router, _storage) = app.into_router_with_state(state);
    for path in [
        "/api/admin/request-logs".to_owned(),
        format!("/api/admin/request-logs/{request_id}"),
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 41000))))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert!(response.status().is_success());
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let value: Value = serde_json::from_slice(&bytes).expect("JSON");
        let cost = value
            .get("request")
            .unwrap_or(&value["items"][0])
            .get("quota_cost")
            .expect("cost");
        assert_eq!(cost["amount_nanos"], "268750000");
        assert_eq!(cost["credits_per_usd"], 25);
        assert_eq!(cost["rate_card"], card.id());
    }
    telemetry.shutdown(Duration::from_secs(1)).await;
}
