use std::collections::BTreeSet;

use any2api_contract_tests::build_public_request_components;
use any2api_domain::{
    CredentialKind, ProtocolDialect, ProtocolOperation, ProviderBaseUrl, ProviderKind, PublicError,
    PublicErrorCode, TokenUsage, TransportMode, UpstreamErrorKind,
};
use any2api_protocol::api::{IngressRequest, ProtocolAdapter, SseFrame, UpstreamResponse};
use any2api_provider::api::{
    OAuthDeviceTokenPoll, OAuthLoginFlow, OAuthProviderEgressStatus, OAuthQuotaRejection,
    ProviderDriver, ProviderRequestHeaderContext, ProviderSecret, UpstreamResponseMeta,
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
    let actual = registry
        .iter()
        .map(|(dialect, _)| *dialect)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual,
        BTreeSet::from([
            ProtocolDialect::OpenAiResponses,
            ProtocolDialect::OpenAiChatCompletions,
            ProtocolDialect::OpenAiImages,
            ProtocolDialect::AnthropicMessages,
        ])
    );

    for (dialect, adapter) in registry.iter() {
        assert_eq!(*dialect, adapter.dialect());
        protocol_local_error_contract(adapter.as_ref());
        match dialect {
            ProtocolDialect::OpenAiResponses => responses_contract(adapter.as_ref()).await,
            ProtocolDialect::OpenAiChatCompletions => {
                chat_completions_contract(adapter.as_ref()).await
            }
            ProtocolDialect::OpenAiImages => images_contract(adapter.as_ref()).await,
            ProtocolDialect::AnthropicMessages => messages_contract(adapter.as_ref()).await,
        }
    }

    let bridges = registry
        .iter_bridges()
        .map(|(dialects, _)| *dialects)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        bridges,
        BTreeSet::from([(
            ProtocolDialect::OpenAiResponses,
            ProtocolDialect::OpenAiChatCompletions,
        )])
    );
    assert!(registry.supports_pair(
        ProtocolDialect::OpenAiResponses,
        ProtocolDialect::OpenAiChatCompletions,
    ));
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
}

fn protocol_local_error_contract(adapter: &dyn ProtocolAdapter) {
    const LOCAL_MESSAGE: &str = "any2api local error detail";
    let response = adapter.error_response(&PublicError::new(
        PublicErrorCode::UpstreamError,
        LOCAL_MESSAGE,
    ));
    let body: Value = serde_json::from_slice(&response.body).expect("protocol error JSON");

    assert_eq!(response.status, StatusCode::BAD_GATEWAY);
    assert_eq!(body["error"]["message"], LOCAL_MESSAGE);
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
        BTreeSet::from([
            ProviderKind::Codex,
            ProviderKind::Claude,
            ProviderKind::Grok,
        ])
    );

    for (kind, driver) in registry.iter() {
        assert_eq!(*kind, driver.kind());
        provider_header_policy_contract(*kind, driver.as_ref());
        provider_error_message_contract(*kind, driver.as_ref());
        match kind {
            ProviderKind::Codex => codex_contract(driver.as_ref()),
            ProviderKind::Claude => claude_contract(driver.as_ref()),
            ProviderKind::Grok => grok_contract(driver.as_ref()),
        }
    }
}

async fn responses_contract(adapter: &dyn ProtocolAdapter) {
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
        .await
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
        .await
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
        TokenUsage::new(Some(12), Some(7), Some(3))
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
        TokenUsage::new(Some(12), Some(7), None)
    );
}

async fn chat_completions_contract(adapter: &dyn ProtocolAdapter) {
    let decoded = adapter
        .decode_ingress_request(ingress_request(
            ProtocolOperation::ChatCompletions,
            "/v1/chat/completions",
            json!({
                "model": "public-model",
                "messages": [],
                "future_field": {"preserved": true}
            }),
        ))
        .await
        .expect("Chat Completions request decodes");
    assert_eq!(decoded.dialect, ProtocolDialect::OpenAiChatCompletions);
    let encoded = adapter
        .encode_upstream_request(
            decoded.operation,
            decoded.headers,
            decoded.payload,
            "upstream-model",
        )
        .expect("Chat Completions request encodes");
    let body: Value = serde_json::from_slice(&encoded.body).expect("encoded JSON");
    assert_eq!(body["model"], "upstream-model");
    assert_eq!(body["future_field"]["preserved"], true);

    let response = adapter
        .decode_upstream_response(UpstreamResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: Bytes::from_static(
                br#"{"usage":{"prompt_tokens":12,"completion_tokens":7,"prompt_tokens_details":{"cached_tokens":3}}}"#,
            ),
        })
        .expect("Chat Completions telemetry decodes");
    assert_eq!(
        response.telemetry.token_usage,
        TokenUsage::new(Some(12), Some(7), Some(3))
    );
    let content = adapter
        .decode_upstream_event(SseFrame(Bytes::from_static(
            b"data: {\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n\n",
        )))
        .expect("Chat Completions content event decodes");
    assert!(content.telemetry().has_content_delta);
}

async fn messages_contract(adapter: &dyn ProtocolAdapter) {
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
        .await
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
        .await
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
        .await
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
        TokenUsage::new(Some(20), Some(9), Some(4))
    );
    let start = adapter
        .decode_upstream_event(SseFrame(Bytes::from_static(
            b"event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":20,\"output_tokens\":1}}}\n\n",
        )))
        .expect("Messages start event decodes");
    assert_eq!(
        start.telemetry().token_usage,
        TokenUsage::new(Some(20), Some(1), None)
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

async fn images_contract(adapter: &dyn ProtocolAdapter) {
    let generated = adapter
        .decode_ingress_request(ingress_request(
            ProtocolOperation::ImagesGenerations,
            "/v1/images/generations",
            json!({
                "model": "gpt-image-2",
                "prompt": "contract image",
                "stream": true,
                "future": {"preserved": true}
            }),
        ))
        .await
        .expect("Images generation request decodes");
    assert_eq!(generated.dialect, ProtocolDialect::OpenAiImages);
    let encoded = adapter
        .encode_upstream_request(
            generated.operation,
            generated.headers,
            generated.payload,
            "upstream-image-model",
        )
        .expect("Images generation request encodes");
    let body: Value = serde_json::from_slice(&encoded.body).expect("Images JSON");
    assert_eq!(body["model"], "upstream-image-model");
    assert_eq!(body["future"]["preserved"], true);
    assert_eq!(encoded.headers[ACCEPT], "text/event-stream");

    let edited = adapter
        .decode_ingress_request(ingress_request(
            ProtocolOperation::ImagesEdits,
            "/v1/images/edits",
            json!({
                "model": "gpt-image-2",
                "prompt": "contract edit",
                "images": [{"image_url": "https://example.com/source.png"}]
            }),
        ))
        .await
        .expect("Images edit request decodes");
    let encoded = adapter
        .encode_upstream_request(
            edited.operation,
            edited.headers,
            edited.payload,
            "upstream-image-model",
        )
        .expect("Images edit request encodes");
    let body: Value = serde_json::from_slice(&encoded.body).expect("Images edit JSON");
    assert_eq!(body["model"], "upstream-image-model");
    assert_eq!(
        body["images"][0]["image_url"],
        "https://example.com/source.png"
    );

    let response = adapter
        .decode_upstream_response(UpstreamResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: Bytes::from_static(
                br#"{"data":[{"b64_json":"abc"}],"usage":{"input_tokens":4,"output_tokens":3}}"#,
            ),
        })
        .expect("Images response decodes");
    assert_eq!(
        response.telemetry.token_usage,
        TokenUsage::new(Some(4), Some(3), None)
    );
    let event = adapter
        .decode_upstream_event(SseFrame(Bytes::from_static(
            b"event: image_generation.completed\ndata: {\"type\":\"image_generation.completed\",\"usage\":{\"input_tokens\":4,\"output_tokens\":3}}\n\n",
        )))
        .expect("Images completion event decodes");
    assert_eq!(
        event.telemetry().token_usage,
        TokenUsage::new(Some(4), Some(3), None)
    );
    assert!(!event.telemetry().has_content_delta);
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
    assert!(
        driver
            .capabilities()
            .protocols
            .contains(&ProtocolDialect::OpenAiImages)
    );
    assert_common_capabilities(driver);
    let plan = driver
        .endpoint_plan(&provider_base_url(), ProtocolOperation::ResponsesCompact)
        .expect("Codex endpoint plan");
    assert_eq!(
        plan.url.as_str(),
        "https://api.example.com/v1/responses/compact"
    );
    let generations = driver
        .endpoint_plan(&provider_base_url(), ProtocolOperation::ImagesGenerations)
        .expect("Codex Images generations endpoint plan");
    assert_eq!(
        generations.url.as_str(),
        "https://api.example.com/v1/images/generations"
    );
    let edits = driver
        .endpoint_plan(&provider_base_url(), ProtocolOperation::ImagesEdits)
        .expect("Codex Images edits endpoint plan");
    assert_eq!(
        edits.url.as_str(),
        "https://api.example.com/v1/images/edits"
    );
    assert!(!driver.oauth_supports_operation(ProtocolOperation::ImagesGenerations));
    assert!(!driver.oauth_supports_operation(ProtocolOperation::ImagesEdits));
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
    let egress = driver
        .oauth_provider_egress_probe_plan()
        .expect("Codex egress probe plan")
        .expect("Codex egress probe");
    assert_eq!(egress.method, Method::GET);
    assert_eq!(
        egress.url.as_str(),
        "https://chatgpt.com/backend-api/wham/usage"
    );
    assert!(!egress.headers.contains_key(AUTHORIZATION));
    assert!(!egress.headers.contains_key("chatgpt-account-id"));
    let forbidden = UpstreamResponseMeta {
        status: StatusCode::FORBIDDEN,
        headers: HeaderMap::new(),
    };
    assert_eq!(
        driver.classify_oauth_quota_rejection(
            &forbidden,
            br#"{"error":{"code":"unsupported_country_region_territory"}}"#,
        ),
        OAuthQuotaRejection::ProviderEgressRestricted
    );
    assert_eq!(
        driver.classify_oauth_provider_egress(&forbidden, b"{}"),
        OAuthProviderEgressStatus::Restricted
    );
}

fn claude_contract(driver: &dyn ProviderDriver) {
    assert!(
        driver
            .capabilities()
            .protocols
            .contains(&ProtocolDialect::AnthropicMessages)
    );
    assert!(
        !driver
            .capabilities()
            .protocols
            .contains(&ProtocolDialect::OpenAiImages)
    );
    assert!(
        driver
            .endpoint_plan(&provider_base_url(), ProtocolOperation::ImagesGenerations)
            .is_err()
    );
    assert!(
        driver
            .endpoint_plan(&provider_base_url(), ProtocolOperation::ImagesEdits)
            .is_err()
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
    assert!(!headers.headers.contains_key("anthropic-version"));
    let identity = driver
        .prepare_request_headers(ProviderRequestHeaderContext {
            ingress_dialect: ProtocolDialect::AnthropicMessages,
            upstream_operation: ProtocolOperation::Messages,
            upstream_model: "claude-contract-model",
            client_headers: &HeaderMap::new(),
            oauth: false,
            allow_credential_bound: true,
            allow_turn_state: false,
        })
        .expect("Claude identity headers");
    assert_eq!(identity["anthropic-version"], "2023-06-01");
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
            .classification()
            .kind(),
        any2api_domain::UpstreamErrorKind::OperationUnavailable
    );
}

fn provider_error_message_contract(kind: ProviderKind, driver: &dyn ProviderDriver) {
    let (operation, status, body, expected_kind, expected_message) = match kind {
        ProviderKind::Codex => (
            ProtocolOperation::Responses,
            StatusCode::BAD_REQUEST,
            br#"{"error":{"type":"invalid_request_error","message":"Official Codex detail"}}"#.as_slice(),
            UpstreamErrorKind::InvalidRequest,
            "Official Codex detail",
        ),
        ProviderKind::Claude => (
            ProtocolOperation::Messages,
            StatusCode::BAD_REQUEST,
            br#"{"type":"error","error":{"type":"invalid_request_error","message":"Official Claude detail"}}"#.as_slice(),
            UpstreamErrorKind::InvalidRequest,
            "Official Claude detail",
        ),
        ProviderKind::Grok => (
            ProtocolOperation::Responses,
            StatusCode::TOO_MANY_REQUESTS,
            br#"{"error":{"code":"subscription:free-usage-exhausted","message":"Official Grok detail"}}"#.as_slice(),
            UpstreamErrorKind::QuotaExhausted,
            "Official Grok detail",
        ),
    };
    let error = driver.classify_error(
        operation,
        &UpstreamResponseMeta {
            status,
            headers: HeaderMap::new(),
        },
        body,
    );

    assert_eq!(error.classification().kind(), expected_kind);
    assert_eq!(error.official_message(), Some(expected_message));
    assert!(!format!("{error:?}").contains(expected_message));

    let invalid = driver.classify_error(
        operation,
        &UpstreamResponseMeta {
            status,
            headers: HeaderMap::new(),
        },
        br#"{"message":"must not be inferred","error":{"message":{"nested":"must not be inferred"}}}"#,
    );
    assert_eq!(invalid.official_message(), None);
}

fn grok_contract(driver: &dyn ProviderDriver) {
    assert!(
        driver
            .capabilities()
            .protocols
            .contains(&ProtocolDialect::OpenAiResponses)
    );
    assert!(
        driver
            .capabilities()
            .protocols
            .contains(&ProtocolDialect::OpenAiChatCompletions)
    );
    assert!(
        !driver
            .capabilities()
            .protocols
            .contains(&ProtocolDialect::OpenAiImages)
    );
    assert!(
        driver
            .endpoint_plan(&provider_base_url(), ProtocolOperation::ImagesGenerations)
            .is_err()
    );
    assert!(
        driver
            .endpoint_plan(&provider_base_url(), ProtocolOperation::ImagesEdits)
            .is_err()
    );
    assert_common_capabilities(driver);
    let compact = driver
        .endpoint_plan(&provider_base_url(), ProtocolOperation::ResponsesCompact)
        .expect("Grok compact endpoint plan");
    assert_eq!(
        compact.url.as_str(),
        "https://api.example.com/v1/responses/compact"
    );
    let chat = driver
        .endpoint_plan(&provider_base_url(), ProtocolOperation::ChatCompletions)
        .expect("Grok chat endpoint plan");
    assert_eq!(
        chat.url.as_str(),
        "https://api.example.com/v1/chat/completions"
    );
    assert_eq!(
        driver
            .credential_test_plan(&provider_base_url())
            .expect("Grok credential test plan")
            .url
            .as_str(),
        "https://api.example.com/v1/models"
    );
    let headers = driver
        .credential_headers(&ProviderSecret::new(1, "xai-contract-key"))
        .expect("Grok credential headers");
    assert_eq!(headers.headers[AUTHORIZATION], "Bearer xai-contract-key");

    assert_eq!(driver.oauth_login_flow(), Some(OAuthLoginFlow::DeviceCode));
    assert_eq!(driver.oauth_redirect_uri(), None);
    assert!(driver.oauth_supports_operation(ProtocolOperation::Responses));
    assert!(!driver.oauth_supports_operation(ProtocolOperation::ResponsesCompact));
    assert!(!driver.oauth_supports_operation(ProtocolOperation::ChatCompletions));
    assert!(!driver.oauth_supports_operation(ProtocolOperation::ImagesGenerations));
    assert!(!driver.oauth_supports_operation(ProtocolOperation::ImagesEdits));
    assert_eq!(
        driver.classify_oauth_quota_rejection(
            &UpstreamResponseMeta {
                status: StatusCode::FORBIDDEN,
                headers: HeaderMap::new(),
            },
            br#"{"code":"unauthorized:blocked-user"}"#,
        ),
        OAuthQuotaRejection::AccountRestricted
    );
    let authorization = driver
        .oauth_device_authorization_request()
        .expect("Grok device authorization request");
    assert_eq!(authorization.url.host_str(), Some("auth.x.ai"));
    assert_eq!(authorization.url.path(), "/oauth2/device/code");
    let device = driver
        .parse_oauth_device_authorization(
            br#"{"device_code":"contract-device-secret","user_code":"ABCD-1234","verification_uri":"https://accounts.x.ai/oauth2/device","expires_in":1800,"interval":5}"#,
        )
        .expect("Grok device authorization response");
    let poll = driver
        .oauth_device_token_request(device.device_code())
        .expect("Grok device token request");
    assert_eq!(poll.url.host_str(), Some("auth.x.ai"));
    assert!(matches!(
        driver
            .parse_oauth_device_token(
                StatusCode::BAD_REQUEST,
                br#"{"error":"authorization_pending"}"#,
            )
            .expect("Grok pending token response"),
        OAuthDeviceTokenPoll::Pending
    ));
    let token = driver
        .parse_oauth_token(
            br#"{"access_token":"grok-oauth-secret","refresh_token":"grok-refresh-secret","sub":"subject-1"}"#,
        )
        .expect("Grok OAuth token");
    let profile = driver
        .oauth_routing_profile(&token)
        .expect("Grok OAuth routing profile");
    assert_eq!(
        profile.base_url().as_str(),
        "https://cli-chat-proxy.grok.com/v1"
    );
    assert_eq!(profile.protocol_dialect(), ProtocolDialect::OpenAiResponses);
    assert!(
        profile
            .models()
            .iter()
            .any(|model| model.as_str() == "grok-4.5")
    );
    let oauth_headers = driver
        .oauth_credential_headers(&token, &HeaderMap::new())
        .expect("Grok OAuth credential headers");
    assert_eq!(
        oauth_headers.headers[AUTHORIZATION],
        "Bearer grok-oauth-secret"
    );
    assert_eq!(oauth_headers.headers["x-userid"], "subject-1");
    assert!(!oauth_headers.headers.contains_key("x-xai-token-auth"));
    let identity = driver
        .prepare_request_headers(ProviderRequestHeaderContext {
            ingress_dialect: ProtocolDialect::OpenAiResponses,
            upstream_operation: ProtocolOperation::Responses,
            upstream_model: "grok-4.5",
            client_headers: &HeaderMap::new(),
            oauth: true,
            allow_credential_bound: true,
            allow_turn_state: false,
        })
        .expect("Grok Build identity headers");
    assert_eq!(identity["x-xai-token-auth"], "xai-grok-cli");
    assert_eq!(identity["x-grok-model-override"], "grok-4.5");
}

fn provider_header_policy_contract(kind: ProviderKind, driver: &dyn ProviderDriver) {
    let (ingress_dialect, operation, upstream_model, default_user_agent) = match kind {
        ProviderKind::Codex => (
            ProtocolDialect::OpenAiResponses,
            ProtocolOperation::Responses,
            "gpt-contract",
            "codex_cli_rs/0.145.0",
        ),
        ProviderKind::Claude => (
            ProtocolDialect::AnthropicMessages,
            ProtocolOperation::Messages,
            "claude-contract",
            "claude-code/2.1.220",
        ),
        ProviderKind::Grok => (
            ProtocolDialect::OpenAiResponses,
            ProtocolOperation::Responses,
            "grok-contract",
            "grok-shell/0.2.112 (macos; aarch64)",
        ),
    };
    let mut client = HeaderMap::new();
    client.insert(
        "user-agent",
        HeaderValue::from_static("official-client/contract"),
    );
    client.insert(
        "traceparent",
        HeaderValue::from_static("00-contract-trace-parent"),
    );
    client.insert("tracestate", HeaderValue::from_static("contract=state"));
    client.insert("connection", HeaderValue::from_static("tracestate"));
    client.insert(
        "authorization",
        HeaderValue::from_static("Bearer gateway-secret"),
    );
    client.insert("x-api-key", HeaderValue::from_static("gateway-secret"));
    client.insert("cookie", HeaderValue::from_static("session=secret"));
    client.insert("x-forwarded-for", HeaderValue::from_static("192.0.2.1"));
    client.insert("x-userid", HeaderValue::from_static("spoofed-account"));
    client.insert("x-unknown-client", HeaderValue::from_static("private"));
    client.insert("originator", HeaderValue::from_static("codex-contract"));
    client.insert("x-codex-beta-features", HeaderValue::from_static("beta"));
    client.insert("x-oai-attestation", HeaderValue::from_static("attestation"));
    client.insert("x-codex-turn-state", HeaderValue::from_static("turn-state"));
    client.insert("x-app", HeaderValue::from_static("claude-contract"));
    client.insert(
        "x-client-request-id",
        HeaderValue::from_static("client-request"),
    );
    client.insert(
        "anthropic-version",
        HeaderValue::from_static("2024-contract"),
    );
    client.insert("x-stainless-runtime", HeaderValue::from_static("contract"));
    client.insert(
        "anthropic-usage-limit",
        HeaderValue::from_static("extended"),
    );
    client.insert(
        "x-grok-client-version",
        HeaderValue::from_static("contract"),
    );
    client.insert("x-grok-conv-id", HeaderValue::from_static("conversation"));

    let request = driver
        .prepare_request_headers(ProviderRequestHeaderContext {
            ingress_dialect,
            upstream_operation: operation,
            upstream_model,
            client_headers: &client,
            oauth: true,
            allow_credential_bound: true,
            allow_turn_state: true,
        })
        .expect("same-dialect provider request headers");
    assert_eq!(request["user-agent"], "official-client/contract");
    assert_eq!(request["traceparent"], "00-contract-trace-parent");
    for forbidden in [
        "authorization",
        "x-api-key",
        "cookie",
        "x-forwarded-for",
        "x-userid",
        "x-unknown-client",
        "tracestate",
    ] {
        assert!(
            !request.contains_key(forbidden),
            "{kind:?} leaked {forbidden}"
        );
    }
    assert_provider_request_headers(kind, &request);

    let cross_dialect = if ingress_dialect == ProtocolDialect::AnthropicMessages {
        ProtocolDialect::OpenAiResponses
    } else {
        ProtocolDialect::AnthropicMessages
    };
    let bridged = driver
        .prepare_request_headers(ProviderRequestHeaderContext {
            ingress_dialect: cross_dialect,
            upstream_operation: operation,
            upstream_model,
            client_headers: &client,
            oauth: true,
            allow_credential_bound: true,
            allow_turn_state: true,
        })
        .expect("cross-dialect provider request headers");
    assert_eq!(bridged["user-agent"], default_user_agent);
    assert!(!bridged.contains_key("traceparent"));

    let upstream = provider_response_header_fixture();
    let response = driver.response_headers(operation, &upstream);
    assert_eq!(response[CONTENT_TYPE], "application/problem+json");
    assert_eq!(response["x-request-id"], "upstream-request");
    assert_eq!(response["request-id"], "provider-request");
    assert_eq!(response["retry-after"], "17");
    for forbidden in [
        "authorization",
        "set-cookie",
        "content-length",
        "x-unknown-upstream",
    ] {
        assert!(
            !response.contains_key(forbidden),
            "{kind:?} leaked {forbidden}"
        );
    }
    assert_provider_response_headers(kind, &response);
}

fn assert_provider_request_headers(kind: ProviderKind, headers: &HeaderMap) {
    match kind {
        ProviderKind::Codex => {
            assert_eq!(headers["originator"], "codex-contract");
            assert_eq!(headers["x-codex-beta-features"], "beta");
            assert_eq!(headers["x-oai-attestation"], "attestation");
            assert_eq!(headers["x-codex-turn-state"], "turn-state");
            assert!(!headers.contains_key("x-app"));
            assert!(!headers.contains_key("x-grok-conv-id"));
        }
        ProviderKind::Claude => {
            assert_eq!(headers["x-app"], "claude-contract");
            assert_eq!(headers["x-client-request-id"], "client-request");
            assert_eq!(headers["anthropic-version"], "2024-contract");
            assert_eq!(headers["x-stainless-runtime"], "contract");
            assert_eq!(headers["anthropic-usage-limit"], "extended");
            assert!(!headers.contains_key("originator"));
            assert!(!headers.contains_key("x-grok-conv-id"));
        }
        ProviderKind::Grok => {
            assert_eq!(headers["x-grok-client-version"], "contract");
            assert_eq!(headers["x-grok-conv-id"], "conversation");
            assert_eq!(headers["x-grok-model-override"], "grok-contract");
            assert_eq!(headers["x-xai-token-auth"], "xai-grok-cli");
            assert_eq!(headers["x-authenticateresponse"], "authenticate-response");
            assert!(!headers.contains_key("originator"));
            assert!(!headers.contains_key("x-app"));
        }
    }
}

fn provider_response_header_fixture() -> HeaderMap {
    let mut upstream = HeaderMap::new();
    upstream.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/problem+json"),
    );
    upstream.insert("x-request-id", HeaderValue::from_static("upstream-request"));
    upstream.insert("request-id", HeaderValue::from_static("provider-request"));
    upstream.insert("retry-after", HeaderValue::from_static("17"));
    upstream.insert(
        "x-oai-request-id",
        HeaderValue::from_static("openai-request"),
    );
    upstream.insert("x-codex-turn-state", HeaderValue::from_static("next-turn"));
    upstream.insert("openai-model", HeaderValue::from_static("gpt-contract"));
    upstream.insert(
        "anthropic-ratelimit-tokens-limit",
        HeaderValue::from_static("1000"),
    );
    upstream.insert(
        "anthropic-usage-limit",
        HeaderValue::from_static("contract"),
    );
    upstream.insert("cf-ray", HeaderValue::from_static("edge-contract"));
    upstream.insert("x-grok-context-window", HeaderValue::from_static("131072"));
    upstream.insert(
        "x-grok-doom-loop-check",
        HeaderValue::from_static("continue"),
    );
    upstream.insert("x-ratelimit-limit-requests", HeaderValue::from_static("60"));
    upstream.insert(
        "authorization",
        HeaderValue::from_static("Bearer upstream-secret"),
    );
    upstream.insert("set-cookie", HeaderValue::from_static("upstream=secret"));
    upstream.insert("content-length", HeaderValue::from_static("123"));
    upstream.insert("x-unknown-upstream", HeaderValue::from_static("private"));
    upstream
}

fn assert_provider_response_headers(kind: ProviderKind, headers: &HeaderMap) {
    match kind {
        ProviderKind::Codex => {
            assert_eq!(headers["x-oai-request-id"], "openai-request");
            assert_eq!(headers["x-codex-turn-state"], "next-turn");
            assert_eq!(headers["openai-model"], "gpt-contract");
            assert_eq!(headers["x-ratelimit-limit-requests"], "60");
            assert!(!headers.contains_key("anthropic-usage-limit"));
            assert!(!headers.contains_key("x-grok-context-window"));
        }
        ProviderKind::Claude => {
            assert_eq!(headers["anthropic-ratelimit-tokens-limit"], "1000");
            assert_eq!(headers["anthropic-usage-limit"], "contract");
            assert_eq!(headers["cf-ray"], "edge-contract");
            assert!(!headers.contains_key("x-oai-request-id"));
            assert!(!headers.contains_key("x-grok-context-window"));
            assert!(!headers.contains_key("x-ratelimit-limit-requests"));
        }
        ProviderKind::Grok => {
            assert_eq!(headers["x-grok-context-window"], "131072");
            assert_eq!(headers["x-grok-doom-loop-check"], "continue");
            assert_eq!(headers["x-ratelimit-limit-requests"], "60");
            assert!(!headers.contains_key("x-oai-request-id"));
            assert!(!headers.contains_key("x-codex-turn-state"));
            assert!(!headers.contains_key("anthropic-usage-limit"));
        }
    }
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
    ProviderBaseUrl::parse("https://api.example.com/v1").expect("provider base URL")
}
