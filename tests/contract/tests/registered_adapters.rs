use std::collections::BTreeSet;

use any2api_contract_tests::build_public_request_components;
use any2api_domain::{
    CredentialKind, ProtocolDialect, ProtocolOperation, ProviderBaseUrl, ProviderKind, PublicError,
    PublicErrorCode, TransportMode, UpstreamErrorKind,
};
use any2api_protocol::api::{
    DecodedResponsePayload, IngressRequest, ProtocolAdapter, UpstreamResponse,
};
use any2api_provider::api::{
    ProviderDriver, ProviderRequestContext, ProviderSecret, UpstreamResponseMeta,
};
use axum::http::{
    HeaderMap, HeaderValue, Method, StatusCode, Uri,
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
};
use bytes::Bytes;
use serde_json::{Value, json};

#[tokio::test]
async fn composition_root_protocol_registry_runs_every_contract() {
    let components = build_public_request_components().expect("public request components");
    let registry = components.protocol_registry();
    assert_eq!(
        registry
            .iter()
            .map(|(dialect, _)| *dialect)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            ProtocolDialect::OpenAiResponses,
            ProtocolDialect::OpenAiChatCompletions,
            ProtocolDialect::OpenAiImages,
            ProtocolDialect::AnthropicMessages,
        ])
    );

    for (dialect, adapter) in registry.iter() {
        assert_eq!(*dialect, adapter.dialect());
        protocol_contract(*dialect, adapter.as_ref()).await;
    }

    assert_eq!(
        registry
            .iter_bridges()
            .map(|(dialects, _)| *dialects)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            (
                ProtocolDialect::OpenAiResponses,
                ProtocolDialect::OpenAiChatCompletions,
            ),
            (
                ProtocolDialect::OpenAiImages,
                ProtocolDialect::OpenAiChatCompletions,
            ),
        ])
    );
    assert!(registry.supports_operation(
        ProtocolDialect::OpenAiResponses,
        ProtocolDialect::OpenAiChatCompletions,
        ProtocolOperation::Responses,
    ));
    assert!(!registry.supports_operation(
        ProtocolDialect::OpenAiResponses,
        ProtocolDialect::OpenAiChatCompletions,
        ProtocolOperation::ResponsesCompact,
    ));
    assert!(registry.supports_operation(
        ProtocolDialect::OpenAiImages,
        ProtocolDialect::OpenAiChatCompletions,
        ProtocolOperation::ImagesGenerations,
    ));
    assert!(!registry.supports_operation(
        ProtocolDialect::OpenAiImages,
        ProtocolDialect::OpenAiChatCompletions,
        ProtocolOperation::ImagesEdits,
    ));
}

async fn protocol_contract(dialect: ProtocolDialect, adapter: &dyn ProtocolAdapter) {
    let (operation, uri, body) = match dialect {
        ProtocolDialect::OpenAiResponses => (
            ProtocolOperation::Responses,
            "/v1/responses",
            json!({"model":"public-model","stream":true,"future_field":42}),
        ),
        ProtocolDialect::OpenAiChatCompletions => (
            ProtocolOperation::ChatCompletions,
            "/v1/chat/completions",
            json!({"model":"public-model","stream":true,"messages":[],"future_field":42}),
        ),
        ProtocolDialect::OpenAiImages => (
            ProtocolOperation::ImagesGenerations,
            "/v1/images/generations",
            json!({"model":"public-model","stream":true,"prompt":"image","future_field":42}),
        ),
        ProtocolDialect::AnthropicMessages => (
            ProtocolOperation::Messages,
            "/v1/messages",
            json!({"model":"public-model","stream":true,"messages":[],"future_field":42}),
        ),
    };
    let decoded = adapter
        .decode_ingress_request(IngressRequest {
            method: Method::POST,
            uri: uri.parse::<Uri>().expect("public URI"),
            headers: HeaderMap::new(),
            body: Bytes::from(serde_json::to_vec(&body).expect("request JSON")),
            operation,
        })
        .await
        .expect("registered protocol decodes its request");
    assert_eq!(decoded.dialect, dialect);
    assert!(decoded.stream);

    let encoded = adapter
        .encode_upstream_request(
            decoded.operation,
            &decoded.headers,
            &decoded.payload,
            "upstream-model",
        )
        .expect("registered protocol encodes its request");
    let body: Value = serde_json::from_slice(&encoded.body).expect("encoded request JSON");
    assert_eq!(body["model"], "upstream-model");
    assert_eq!(body["future_field"], 42);
    assert_eq!(encoded.headers[ACCEPT], "text/event-stream");

    let response = adapter.error_response(&PublicError::new(
        PublicErrorCode::UpstreamError,
        "local contract error",
    ));
    let body: Value = serde_json::from_slice(&response.body).expect("protocol error JSON");
    assert_eq!(response.status, StatusCode::BAD_GATEWAY);
    assert_eq!(body["error"]["message"], "local contract error");

    let response_body = match dialect {
        ProtocolDialect::OpenAiResponses => json!({
            "id": "resp_contract",
            "model": "public-model",
            "status": "completed",
            "output": [],
            "usage": {"input_tokens": 1, "output_tokens": 2}
        }),
        ProtocolDialect::OpenAiChatCompletions => json!({
            "id": "chatcmpl_contract",
            "model": "public-model",
            "choices": [],
            "usage": {"prompt_tokens": 1, "completion_tokens": 2}
        }),
        ProtocolDialect::OpenAiImages => json!({
            "created": 1,
            "model": "public-model",
            "data": [],
            "usage": {"input_tokens": 1, "output_tokens": 2}
        }),
        ProtocolDialect::AnthropicMessages => json!({
            "id": "msg_contract",
            "type": "message",
            "role": "assistant",
            "model": "public-model",
            "content": [],
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 1, "output_tokens": 2}
        }),
    };
    let wire = Bytes::from(serde_json::to_vec(&response_body).expect("response JSON"));
    let wire_pointer = wire.as_ptr();
    let decoded_response = adapter
        .decode_direct_upstream_response(UpstreamResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: wire.clone(),
        })
        .expect("registered protocol directly decodes buffered JSON");
    match &decoded_response.payload {
        DecodedResponsePayload::RawJson(body) => {
            assert_eq!(body.as_ptr(), wire_pointer);
            assert_eq!(body, &wire);
        }
        DecodedResponsePayload::StructuredJson(_) => {
            panic!("registered direct protocol path materialized a JSON value")
        }
    }
    let egress = adapter
        .encode_egress_response(decoded_response, "public-model")
        .expect("registered protocol encodes its direct response");
    assert_eq!(egress.body.as_ptr(), wire_pointer);
    assert_eq!(egress.body, wire);
}

#[test]
fn composition_root_provider_registry_runs_every_contract() {
    let components = build_public_request_components().expect("public request components");
    let registry = components.provider_registry();
    assert_eq!(
        registry
            .iter()
            .map(|(kind, _)| *kind)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            ProviderKind::Codex,
            ProviderKind::Claude,
            ProviderKind::Grok,
            ProviderKind::Kimi,
        ])
    );

    for (kind, driver) in registry.iter() {
        assert_eq!(*kind, driver.kind());
        provider_contract(*kind, driver.as_ref());
    }
}

fn provider_contract(kind: ProviderKind, driver: &dyn ProviderDriver) {
    let fixture = ProviderFixture::for_kind(kind);
    let capabilities = driver.capabilities();
    assert_eq!(capabilities.protocols, fixture.protocols);
    assert_eq!(
        capabilities.transport_modes,
        BTreeSet::from([TransportMode::Json, TransportMode::Sse])
    );
    assert_eq!(
        capabilities.credential_kinds,
        BTreeSet::from([CredentialKind::ApiKey])
    );

    let plan = driver
        .endpoint_plan(&fixture.base_url, fixture.operation)
        .expect("registered provider builds its representative endpoint");
    assert_eq!(plan.url.as_str(), fixture.expected_url);

    let secret = ProviderSecret::new(fixture.secret);
    driver
        .validate_credential(&secret)
        .expect("registered provider accepts a non-empty API key");
    let credential_headers = driver
        .credential_headers(&fixture.base_url, &secret)
        .expect("registered provider builds API key headers");
    assert_eq!(
        credential_headers.headers[AUTHORIZATION],
        format!("Bearer {}", fixture.secret)
    );

    let client_headers = client_headers(fixture.safe_header);
    let request_context = ProviderRequestContext {
        ingress_dialect: fixture.ingress_dialect,
        upstream_operation: fixture.operation,
        upstream_model: "contract-model",
        client_headers: &client_headers,
        oauth: false,
        allow_credential_bound: true,
        allow_turn_state: true,
    };
    let request_body = Bytes::from_static(br#"{"future_field":42}"#);
    let prepared_body = driver
        .prepare_request_body(request_context, request_body.clone())
        .expect("registered provider prepares API key request bodies");
    assert_eq!(prepared_body.as_ptr(), request_body.as_ptr());
    assert_eq!(prepared_body, request_body);
    let request_headers = driver
        .prepare_request_headers(request_context)
        .expect("registered provider projects request headers");
    if kind == ProviderKind::Kimi {
        assert!(request_headers.is_empty());
    } else {
        assert_eq!(request_headers["user-agent"], "official-client/contract");
        assert_eq!(request_headers["traceparent"], "00-contract-trace");
        assert_eq!(request_headers[fixture.safe_header], "safe-provider-value");
    }
    for forbidden in [
        "authorization",
        "x-api-key",
        "cookie",
        "x-forwarded-for",
        "x-unknown-client",
    ] {
        assert!(
            !request_headers.contains_key(forbidden),
            "{kind:?} leaked {forbidden}"
        );
    }

    let response_headers = driver.response_headers(fixture.operation, &upstream_headers());
    assert_eq!(response_headers[CONTENT_TYPE], "application/problem+json");
    assert_eq!(response_headers["x-request-id"], "upstream-request");
    assert_eq!(response_headers["retry-after"], "17");
    for forbidden in ["authorization", "set-cookie", "content-length"] {
        assert!(
            !response_headers.contains_key(forbidden),
            "{kind:?} leaked {forbidden}"
        );
    }

    let error = driver.classify_error(
        fixture.operation,
        &UpstreamResponseMeta {
            status: fixture.error_status,
            headers: HeaderMap::new(),
        },
        fixture.error_body,
    );
    assert_eq!(error.classification().kind(), fixture.error_kind);
    assert_eq!(error.official_message(), Some(fixture.error_message));
}

struct ProviderFixture {
    protocols: BTreeSet<ProtocolDialect>,
    base_url: ProviderBaseUrl,
    ingress_dialect: ProtocolDialect,
    operation: ProtocolOperation,
    expected_url: &'static str,
    secret: &'static str,
    safe_header: &'static str,
    error_status: StatusCode,
    error_body: &'static [u8],
    error_kind: UpstreamErrorKind,
    error_message: &'static str,
}

impl ProviderFixture {
    fn for_kind(kind: ProviderKind) -> Self {
        match kind {
            ProviderKind::Codex => Self {
                protocols: BTreeSet::from([
                    ProtocolDialect::OpenAiResponses,
                    ProtocolDialect::OpenAiChatCompletions,
                    ProtocolDialect::OpenAiImages,
                ]),
                base_url: base_url("https://api.example.com/v1"),
                ingress_dialect: ProtocolDialect::OpenAiResponses,
                operation: ProtocolOperation::ResponsesCompact,
                expected_url: "https://api.example.com/v1/responses/compact",
                secret: "codex-contract-key",
                safe_header: "originator",
                error_status: StatusCode::BAD_REQUEST,
                error_body: br#"{"error":{"type":"invalid_request_error","message":"Codex detail"}}"#,
                error_kind: UpstreamErrorKind::InvalidRequest,
                error_message: "Codex detail",
            },
            ProviderKind::Claude => Self {
                protocols: BTreeSet::from([ProtocolDialect::AnthropicMessages]),
                base_url: base_url("https://api.example.com/gateway"),
                ingress_dialect: ProtocolDialect::AnthropicMessages,
                operation: ProtocolOperation::MessagesCountTokens,
                expected_url: "https://api.example.com/gateway/v1/messages/count_tokens",
                secret: "claude-contract-key",
                safe_header: "x-app",
                error_status: StatusCode::BAD_REQUEST,
                error_body: br#"{"type":"error","error":{"type":"invalid_request_error","message":"Claude detail"}}"#,
                error_kind: UpstreamErrorKind::InvalidRequest,
                error_message: "Claude detail",
            },
            ProviderKind::Grok => Self {
                protocols: BTreeSet::from([
                    ProtocolDialect::OpenAiResponses,
                    ProtocolDialect::OpenAiChatCompletions,
                ]),
                base_url: base_url("https://api.example.com/v1"),
                ingress_dialect: ProtocolDialect::OpenAiChatCompletions,
                operation: ProtocolOperation::ChatCompletions,
                expected_url: "https://api.example.com/v1/chat/completions",
                secret: "grok-contract-key",
                safe_header: "x-grok-client-version",
                error_status: StatusCode::TOO_MANY_REQUESTS,
                error_body: br#"{"error":{"code":"subscription:free-usage-exhausted","message":"Grok detail"}}"#,
                error_kind: UpstreamErrorKind::QuotaExhausted,
                error_message: "Grok detail",
            },
            ProviderKind::Kimi => Self {
                protocols: BTreeSet::from([ProtocolDialect::OpenAiChatCompletions]),
                base_url: base_url("https://api.moonshot.cn/v1"),
                ingress_dialect: ProtocolDialect::OpenAiResponses,
                operation: ProtocolOperation::ChatCompletions,
                expected_url: "https://api.moonshot.cn/v1/chat/completions",
                secret: "kimi-contract-key",
                safe_header: "x-grok-client-version",
                error_status: StatusCode::TOO_MANY_REQUESTS,
                error_body: br#"{"error":{"type":"exceeded_current_quota_error","message":"Kimi detail"}}"#,
                error_kind: UpstreamErrorKind::QuotaExhausted,
                error_message: "Kimi detail",
            },
        }
    }
}

fn client_headers(safe_header: &'static str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "user-agent",
        HeaderValue::from_static("official-client/contract"),
    );
    headers.insert("traceparent", HeaderValue::from_static("00-contract-trace"));
    headers.insert(safe_header, HeaderValue::from_static("safe-provider-value"));
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_static("Bearer gateway-secret"),
    );
    headers.insert("x-api-key", HeaderValue::from_static("gateway-secret"));
    headers.insert("cookie", HeaderValue::from_static("session=secret"));
    headers.insert("x-forwarded-for", HeaderValue::from_static("192.0.2.1"));
    headers.insert("x-unknown-client", HeaderValue::from_static("private"));
    headers
}

fn upstream_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/problem+json"),
    );
    headers.insert("x-request-id", HeaderValue::from_static("upstream-request"));
    headers.insert("retry-after", HeaderValue::from_static("17"));
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_static("Bearer upstream-secret"),
    );
    headers.insert("set-cookie", HeaderValue::from_static("upstream=secret"));
    headers.insert("content-length", HeaderValue::from_static("123"));
    headers
}

fn base_url(value: &str) -> ProviderBaseUrl {
    ProviderBaseUrl::parse(value).expect("provider base URL")
}
