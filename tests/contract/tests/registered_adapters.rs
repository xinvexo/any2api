use std::collections::BTreeSet;

use any2api_contract_tests::build_public_request_components;
use any2api_domain::{
    CredentialKind, ErrorClass, ProtocolDialect, ProtocolOperation, ProviderBaseUrl, ProviderKind,
    TransportMode,
};
use any2api_protocol::api::{IngressRequest, ProtocolAdapter, SseFrame};
use any2api_provider::api::{ProviderDriver, ProviderSecret, UpstreamResponseMeta};
use axum::http::{
    HeaderMap, Method, StatusCode, Uri,
    header::{ACCEPT, AUTHORIZATION},
};
use bytes::Bytes;
use serde_json::{Value, json};

#[test]
fn composition_root_protocol_registry_runs_every_contract() {
    let components = build_public_request_components().expect("public request components");
    let registry = components.protocol_registry();
    let actual = registry
        .iter()
        .map(|(dialect, _)| *dialect)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual,
        BTreeSet::from([
            ProtocolDialect::OpenAiResponses,
            ProtocolDialect::AnthropicMessages,
        ])
    );

    for (dialect, adapter) in registry.iter() {
        assert_eq!(*dialect, adapter.dialect());
        match dialect {
            ProtocolDialect::OpenAiResponses => responses_contract(adapter.as_ref()),
            ProtocolDialect::AnthropicMessages => messages_contract(adapter.as_ref()),
            ProtocolDialect::CodexBackend => {
                panic!("registered Codex Backend adapter has no first-release contract")
            }
        }
    }
}

#[test]
fn composition_root_provider_registry_runs_every_contract() {
    let components = build_public_request_components().expect("public request components");
    let registry = components.provider_registry();
    let actual = registry
        .iter()
        .map(|(kind, _)| *kind)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual,
        BTreeSet::from([ProviderKind::Codex, ProviderKind::Claude])
    );

    for (kind, driver) in registry.iter() {
        assert_eq!(*kind, driver.kind());
        match kind {
            ProviderKind::Codex => codex_contract(driver.as_ref()),
            ProviderKind::Claude => claude_contract(driver.as_ref()),
        }
    }
}

fn responses_contract(adapter: &dyn ProtocolAdapter) {
    let decoded = adapter
        .decode_ingress_request(ingress_request(
            ProtocolOperation::Responses,
            "/v1/responses",
            json!({
                "model": "public-model",
                "stream": false,
                "future_field": {"preserved": true}
            }),
        ))
        .expect("Responses request decodes");
    assert_eq!(decoded.dialect, ProtocolDialect::OpenAiResponses);
    let encoded = adapter
        .encode_upstream_request(
            decoded.operation,
            decoded.headers,
            decoded.payload,
            "upstream-model",
        )
        .expect("Responses request encodes");
    let body: Value = serde_json::from_slice(&encoded.body).expect("encoded JSON");
    assert_eq!(body["model"], "upstream-model");
    assert_eq!(body["future_field"]["preserved"], true);

    let streaming = adapter
        .decode_ingress_request(ingress_request(
            ProtocolOperation::Responses,
            "/v1/responses",
            json!({"model":"public-model","stream":true}),
        ))
        .expect("streaming Responses request decodes");
    assert!(streaming.stream);
    let streaming = adapter
        .encode_upstream_request(
            streaming.operation,
            streaming.headers,
            streaming.payload,
            "upstream-model",
        )
        .expect("streaming Responses request encodes");
    assert_eq!(streaming.headers[ACCEPT], "text/event-stream");
    assert_stream_model_rewrite(
        adapter,
        b"event: response.created\ndata: {\"response\":{\"model\":\"upstream-model\"}}\n\n",
    );
}

fn messages_contract(adapter: &dyn ProtocolAdapter) {
    let decoded = adapter
        .decode_ingress_request(ingress_request(
            ProtocolOperation::Messages,
            "/v1/messages",
            json!({
                "model": "public-model",
                "messages": [],
                "future_field": 42
            }),
        ))
        .expect("Messages request decodes");
    assert_eq!(decoded.dialect, ProtocolDialect::AnthropicMessages);
    let encoded = adapter
        .encode_upstream_request(
            decoded.operation,
            decoded.headers,
            decoded.payload,
            "upstream-model",
        )
        .expect("Messages request encodes");
    let body: Value = serde_json::from_slice(&encoded.body).expect("encoded JSON");
    assert_eq!(body["model"], "upstream-model");
    assert_eq!(body["future_field"], 42);

    let streaming = adapter
        .decode_ingress_request(ingress_request(
            ProtocolOperation::Messages,
            "/v1/messages",
            json!({"model":"public-model","stream":true,"messages":[]}),
        ))
        .expect("streaming Messages request decodes");
    assert!(streaming.stream);
    let streaming = adapter
        .encode_upstream_request(
            streaming.operation,
            streaming.headers,
            streaming.payload,
            "upstream-model",
        )
        .expect("streaming Messages request encodes");
    assert_eq!(streaming.headers[ACCEPT], "text/event-stream");
    assert_stream_model_rewrite(
        adapter,
        b"event: message_start\ndata: {\"message\":{\"model\":\"upstream-model\"}}\n\n",
    );

    let count_tokens = adapter
        .decode_ingress_request(ingress_request(
            ProtocolOperation::MessagesCountTokens,
            "/v1/messages/count_tokens",
            json!({"model":"public-model","messages":[],"future_count_field":true}),
        ))
        .expect("Count Tokens request decodes");
    let count_tokens = adapter
        .encode_upstream_request(
            count_tokens.operation,
            count_tokens.headers,
            count_tokens.payload,
            "upstream-model",
        )
        .expect("Count Tokens request encodes");
    let body: Value = serde_json::from_slice(&count_tokens.body).expect("encoded JSON");
    assert_eq!(body["model"], "upstream-model");
    assert_eq!(body["future_count_field"], true);
}

fn ingress_request(operation: ProtocolOperation, uri: &'static str, body: Value) -> IngressRequest {
    IngressRequest {
        method: Method::POST,
        uri: Uri::from_static(uri),
        headers: HeaderMap::new(),
        body: Bytes::from(serde_json::to_vec(&body).expect("request JSON")),
        operation,
    }
}

fn assert_stream_model_rewrite(adapter: &dyn ProtocolAdapter, frame: &'static [u8]) {
    let event = adapter
        .decode_upstream_event(SseFrame(Bytes::from_static(frame)))
        .expect("stream event decodes");
    let frame = adapter
        .encode_egress_event(event, "public-model")
        .expect("stream event encodes");
    let text = String::from_utf8_lossy(&frame.0);
    assert!(text.contains(r#""model":"public-model""#));
    assert!(!text.contains("upstream-model"));
}

fn codex_contract(driver: &dyn ProviderDriver) {
    assert!(
        driver
            .capabilities()
            .protocols
            .contains(&ProtocolDialect::OpenAiResponses)
    );
    assert_common_capabilities(driver);
    let plan = driver
        .endpoint_plan(&provider_base_url(), ProtocolOperation::ResponsesCompact)
        .expect("Codex endpoint plan");
    assert_eq!(
        plan.url.as_str(),
        "https://api.example.com/v1/responses/compact"
    );
    let headers = driver
        .credential_headers(&ProviderSecret::new(1, "sk-codex-contract"))
        .expect("Codex credential headers");
    assert_eq!(headers.headers[AUTHORIZATION], "Bearer sk-codex-contract");
}

fn claude_contract(driver: &dyn ProviderDriver) {
    assert!(
        driver
            .capabilities()
            .protocols
            .contains(&ProtocolDialect::AnthropicMessages)
    );
    assert_common_capabilities(driver);
    let plan = driver
        .endpoint_plan(&provider_base_url(), ProtocolOperation::MessagesCountTokens)
        .expect("Claude endpoint plan");
    assert_eq!(
        plan.url.as_str(),
        "https://api.example.com/v1/messages/count_tokens"
    );
    let headers = driver
        .credential_headers(&ProviderSecret::new(1, "sk-claude-contract"))
        .expect("Claude credential headers");
    assert_eq!(headers.headers["x-api-key"], "sk-claude-contract");
    assert_eq!(headers.headers["anthropic-version"], "2023-06-01");
    assert_eq!(
        driver.classify_error(
            ProtocolOperation::MessagesCountTokens,
            &UpstreamResponseMeta {
                status: StatusCode::NOT_FOUND,
                headers: HeaderMap::new(),
            },
            b"{}",
        ),
        ErrorClass::OperationUnavailable
    );
}

fn assert_common_capabilities(driver: &dyn ProviderDriver) {
    let capabilities = driver.capabilities();
    assert!(capabilities.transport_modes.contains(&TransportMode::Json));
    assert!(capabilities.transport_modes.contains(&TransportMode::Sse));
    assert!(
        capabilities
            .credential_kinds
            .contains(&CredentialKind::ApiKey)
    );
}

fn provider_base_url() -> ProviderBaseUrl {
    ProviderBaseUrl::parse("https://api.example.com/v1", false, false).expect("provider base URL")
}
