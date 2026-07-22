use std::collections::BTreeSet;

use any2api_contract_tests::build_public_request_components;
use any2api_domain::{
    CredentialKind, ProtocolDialect, ProtocolOperation, ProviderBaseUrl, ProviderKind, TokenUsage,
    TransportMode,
};
use any2api_protocol::api::{IngressRequest, ProtocolAdapter, SseFrame, UpstreamResponse};
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

    let response = adapter
        .decode_upstream_response(UpstreamResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: Bytes::from_static(
                br#"{"usage":{"input_tokens":12,"output_tokens":7,"input_tokens_details":{"cached_tokens":3,"cache_write_tokens":2}}}"#,
            ),
        })
        .expect("Responses telemetry decodes");
    assert_eq!(
        response.telemetry.token_usage,
        TokenUsage::new(Some(12), Some(7), Some(3), Some(2))
    );
    let content = adapter
        .decode_upstream_event(SseFrame(Bytes::from_static(
            b"event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"hello\"}\n\n",
        )))
        .expect("Responses content event decodes");
    assert!(content.telemetry().has_content_delta);
    let terminal = adapter
        .decode_upstream_event(SseFrame(Bytes::from_static(
            b"event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":12,\"output_tokens\":7}}}\n\n",
        )))
        .expect("Responses terminal event decodes");
    assert_eq!(
        terminal.telemetry().token_usage,
        TokenUsage::new(Some(12), Some(7), None, None)
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

    let response = adapter
        .decode_upstream_response(UpstreamResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: Bytes::from_static(
                br#"{"usage":{"input_tokens":20,"output_tokens":9,"cache_read_input_tokens":4,"cache_creation_input_tokens":3}}"#,
            ),
        })
        .expect("Messages telemetry decodes");
    assert_eq!(
        response.telemetry.token_usage,
        TokenUsage::new(Some(20), Some(9), Some(4), Some(3))
    );
    let start = adapter
        .decode_upstream_event(SseFrame(Bytes::from_static(
            b"event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":20,\"output_tokens\":1}}}\n\n",
        )))
        .expect("Messages start event decodes");
    assert_eq!(
        start.telemetry().token_usage,
        TokenUsage::new(Some(20), Some(1), None, None)
    );
    let content = adapter
        .decode_upstream_event(SseFrame(Bytes::from_static(
            b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"hello\"}}\n\n",
        )))
        .expect("Messages content event decodes");
    assert!(content.telemetry().has_content_delta);
    let count_response = adapter
        .decode_upstream_response(UpstreamResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: Bytes::from_static(br#"{"input_tokens":37}"#),
        })
        .expect("Count Tokens response decodes");
    assert_eq!(count_response.telemetry.token_usage, TokenUsage::default());
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
    assert_eq!(
        driver
            .credential_test_plan(&provider_base_url())
            .expect("Codex credential test plan")
            .url
            .as_str(),
        "https://api.example.com/v1/models"
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
    assert_eq!(
        driver
            .credential_test_plan(&provider_base_url())
            .expect("Claude credential test plan")
            .url
            .as_str(),
        "https://api.example.com/v1/models"
    );
    let headers = driver
        .credential_headers(&ProviderSecret::new(1, "sk-claude-contract"))
        .expect("Claude credential headers");
    assert_eq!(headers.headers["x-api-key"], "sk-claude-contract");
    assert_eq!(headers.headers["anthropic-version"], "2023-06-01");
    assert_eq!(
        driver
            .classify_error(
                ProtocolOperation::MessagesCountTokens,
                &UpstreamResponseMeta {
                    status: StatusCode::NOT_FOUND,
                    headers: HeaderMap::new(),
                },
                b"{}",
            )
            .kind(),
        any2api_domain::UpstreamErrorKind::OperationUnavailable
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
