use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

use any2api_domain::{
    CompletedRequestLog, CredentialId, CredentialKind, ErrorClass, GatewayApiKeyId,
    ProtocolDialect, ProtocolOperation, ProviderCredentialDraft, ProviderEndpointDraft,
    ProviderEndpointId, ProviderKind, ProxyProfileId, RateLimitMode, RequestAttemptOutcome,
    RequestId, RetrySafety, SettingKey, SettingValue,
};
use any2api_protocol::{
    AnthropicMessagesAdapter, OpenAiChatCompletionsAdapter, OpenAiImagesAdapter,
    OpenAiResponsesAdapter, ProtocolRegistry, ResponsesToChatCompletionsBridge,
};
use any2api_runtime::api::{
    ConfigPublisher, ProviderApiKeySecret, PublicRequest, PublicRequestService, PublicResponse,
    PublicResponseBody, PublishedSnapshot, RequestTelemetry, RuntimeRegistry, SnapshotStore,
};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use any2api_transport::api::{
    BoxByteStream, TransportError, TransportErrorStage, TransportFailureScope, TransportManager,
    TransportProxy, TransportRequest, TransportResponse,
};
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::{StreamExt, stream};
use http::{HeaderMap, HeaderValue, StatusCode, header};
use serde_json::{Value, json};
use tempfile::TempDir;

#[tokio::test]
async fn definitely_not_sent_failure_switches_to_another_credential() {
    let transport = Arc::new(ScriptedTransport::new([
        ScriptStep::Error(TransportError::new(
            TransportErrorStage::Tcp,
            TransportFailureScope::Endpoint,
            RetrySafety::DefinitelyNotSent,
            "connection refused",
        )),
        ScriptStep::json(
            StatusCode::OK,
            r#"{"id":"retry-ok","model":"upstream","output":[]}"#,
        ),
    ]));
    let harness = harness(transport.clone(), 2, &["retry-model"], &[]).await;

    let response = execute_json(&harness, "retry-model", json!({"input":"hello"})).await;

    assert_eq!(response.status(), StatusCode::OK);
    let calls = transport.calls();
    assert_eq!(calls.len(), 2);
    assert_ne!(calls[0].uri, calls[1].uri);
    assert_ne!(calls[0].authorization, calls[1].authorization);
    let record = wait_for_log(&harness, response.request_id).await;
    assert_eq!(record.request.status_code, 200);
    assert_eq!(record.request.error_class, None);
    assert_eq!(record.request.attempt_count, 2);
    assert_eq!(record.attempts.len(), 2);
    assert_eq!(record.attempts[0].attempt_no, 1);
    assert_eq!(
        record.attempts[0].outcome,
        RequestAttemptOutcome::TransportError
    );
    assert_eq!(
        record.attempts[0].retry_safety,
        Some(RetrySafety::DefinitelyNotSent)
    );
    assert_eq!(record.attempts[0].error_class, Some(ErrorClass::Network));
    assert_eq!(record.attempts[0].status_code, None);
    assert_eq!(record.attempts[1].attempt_no, 2);
    assert_eq!(record.attempts[1].outcome, RequestAttemptOutcome::Success);
    assert_eq!(record.attempts[1].retry_safety, None);
    assert_eq!(record.attempts[1].error_class, None);
    assert_eq!(record.attempts[1].status_code, Some(200));
    assert_ne!(
        record.attempts[0].credential_id,
        record.attempts[1].credential_id
    );
    assert_ne!(
        record.attempts[0].route_target_id,
        record.attempts[1].route_target_id
    );
    assert_eq!(
        record.request.credential_id,
        record.attempts[1].credential_id
    );
    harness.telemetry.shutdown(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn session_retry_does_not_replay_turn_state_to_another_credential() {
    let transport = Arc::new(ScriptedTransport::new([
        failure_step(),
        ScriptStep::json(
            StatusCode::OK,
            r#"{"id":"retry-session","model":"upstream","output":[]}"#,
        ),
        ScriptStep::json(
            StatusCode::OK,
            r#"{"id":"retry-session-follow","model":"upstream","output":[]}"#,
        ),
    ]));
    let harness = harness(
        transport.clone(),
        2,
        &["session-retry-model"],
        &[(SettingKey::AffinityEnabled, SettingValue::Boolean(true))],
    )
    .await;
    let mut headers = HeaderMap::new();
    headers.insert("session-id", HeaderValue::from_static("session-retry"));
    headers.insert(
        "x-codex-turn-state",
        HeaderValue::from_static("credential-zero-state"),
    );

    let response = execute_operation_with_headers(
        &harness,
        ProtocolOperation::Responses,
        "session-retry-model",
        json!({"input":"hello"}),
        headers,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let mut follow_headers = HeaderMap::new();
    follow_headers.insert("session-id", HeaderValue::from_static("session-retry"));
    follow_headers.insert(
        "x-codex-turn-state",
        HeaderValue::from_static("final-credential-state"),
    );
    let follow = execute_operation_with_headers(
        &harness,
        ProtocolOperation::Responses,
        "session-retry-model",
        json!({"input":"follow"}),
        follow_headers,
    )
    .await;
    assert_eq!(follow.status(), StatusCode::OK);

    let calls = transport.calls();
    assert_eq!(calls.len(), 3);
    assert_ne!(calls[0].authorization, calls[1].authorization);
    assert_eq!(calls[2].authorization, calls[1].authorization);
    assert_eq!(calls[0].turn_state, None);
    assert_eq!(calls[1].turn_state, None);
    assert_eq!(
        calls[2].turn_state.as_deref(),
        Some("final-credential-state")
    );
    harness.telemetry.shutdown(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn buffered_response_persists_exact_usage_without_inventing_ttft() {
    let transport = Arc::new(ScriptedTransport::new([ScriptStep::json(
        StatusCode::OK,
        r#"{"id":"usage-json","model":"upstream","output":[],"usage":{"input_tokens":120,"output_tokens":45,"input_tokens_details":{"cached_tokens":30,"cache_write_tokens":6}}}"#,
    )]));
    let harness = harness(transport, 1, &["usage-json-model"], &[]).await;

    let response = execute_json(&harness, "usage-json-model", json!({"input":"hello"})).await;

    assert_eq!(response.status(), StatusCode::OK);
    let record = wait_for_log(&harness, response.request_id).await;
    assert_eq!(record.request.first_token_ms, None);
    assert_eq!(record.request.input_tokens, Some(120));
    assert_eq!(record.request.output_tokens, Some(45));
    assert_eq!(record.request.cache_read_tokens, Some(30));
    harness.telemetry.shutdown(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn responses_compact_persists_exact_usage_without_ttft() {
    let transport = Arc::new(ScriptedTransport::new([ScriptStep::json(
        StatusCode::OK,
        r#"{"output":[{"type":"compaction","encrypted_content":"opaque"}],"usage":{"input_tokens":70,"output_tokens":5,"input_tokens_details":{"cached_tokens":10,"cache_write_tokens":2}}}"#,
    )]));
    let harness = harness(transport, 1, &["compact-usage-model"], &[]).await;

    let response = execute_operation(
        &harness,
        ProtocolOperation::ResponsesCompact,
        "compact-usage-model",
        json!({"input":[]}),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let record = wait_for_log(&harness, response.request_id).await;
    assert_eq!(record.request.first_token_ms, None);
    assert_eq!(record.request.input_tokens, Some(70));
    assert_eq!(record.request.output_tokens, Some(5));
    assert_eq!(record.request.cache_read_tokens, Some(10));
    harness.telemetry.shutdown(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn remote_compaction_v2_accepts_a_large_first_event_and_finishes_on_terminal() {
    const LARGE_COMPACTION_CONTENT_BYTES: usize = 300 * 1024;
    let encrypted_content = "x".repeat(LARGE_COMPACTION_CONTENT_BYTES);
    let compaction = format!(
        "event: response.output_item.done\ndata: {{\"type\":\"response.output_item.done\",\"sequence_number\":9,\"output_index\":0,\"item\":{{\"id\":\"cmp_1\",\"type\":\"compaction_summary\",\"encrypted_content\":\"{encrypted_content}\",\"future_item_field\":{{\"keep\":[1,2]}}}},\"future_event_field\":{{\"keep\":true}}}}\n\n"
    );
    let transport = Arc::new(ScriptedTransport::new([
        ScriptStep::DelayedStalledStreamWithGap {
            first_delay: Duration::from_secs(84),
            first: Bytes::from(compaction),
            second_delay: Duration::from_secs(61),
            second: Bytes::from_static(
                b"event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"compact-v2-id\",\"model\":\"upstream\",\"usage\":{\"input_tokens\":241753,\"output_tokens\":0}},\"future_terminal_field\":true}\n\n",
            ),
        },
    ]));
    let harness = harness(transport.clone(), 1, &["compact-v2-model"], &[]).await;
    tokio::time::pause();
    let request_id = RequestId::new();

    let response = execute_stream_operation(
        &harness,
        request_id,
        ProtocolOperation::Responses,
        "compact-v2-model",
        json!({
            "input": [
                {"type":"message","role":"user","content":"long history"},
                {"type":"compaction_trigger"}
            ],
            "reasoning":{"effort":"high","summary":"auto"}
        }),
    )
    .await;
    let mut body = streaming_body(response);
    let mut emitted = Vec::new();
    while let Some(frame) = body.next().await {
        emitted.extend_from_slice(&frame.expect("valid compaction SSE frame"));
    }
    let emitted = String::from_utf8(emitted).expect("SSE is UTF-8");
    assert!(emitted.contains("response.output_item.done"));
    assert!(emitted.contains(r#""type":"compaction_summary""#));
    assert!(emitted.len() > LARGE_COMPACTION_CONTENT_BYTES);
    assert!(emitted.contains(r#""encrypted_content":"xxxxxxxx"#));
    assert!(emitted.contains(r#""future_item_field":{"keep":[1,2]}"#));
    assert!(emitted.contains(r#""future_event_field":{"keep":true}"#));
    assert!(emitted.contains(r#""future_terminal_field":true"#));
    assert!(emitted.contains("response.completed"));

    let calls = transport.calls();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].uri.ends_with("/v1/responses"));
    assert_eq!(calls[0].read_timeout, Duration::from_secs(300));
    let upstream_body: Value =
        serde_json::from_slice(&calls[0].body).expect("upstream request JSON");
    assert_eq!(
        upstream_body["input"]
            .as_array()
            .and_then(|items| items.last()),
        Some(&json!({"type":"compaction_trigger"}))
    );
    assert_eq!(
        upstream_body["reasoning"],
        json!({"effort":"high","summary":"auto"})
    );
    tokio::time::resume();
    let record = wait_for_log(&harness, request_id).await;
    assert_eq!(record.request.error_class, None);
    assert_eq!(record.attempts[0].outcome, RequestAttemptOutcome::Success);
    harness.telemetry.shutdown(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn responses_compact_survives_a_delayed_unary_body() {
    let transport = Arc::new(ScriptedTransport::new([ScriptStep::delayed_json(
        Duration::from_secs(30),
        StatusCode::OK,
        r#"{"output":[{"type":"compaction","encrypted_content":"opaque"}]}"#,
    )]));
    let harness = harness(transport.clone(), 1, &["compact-model"], &[]).await;
    tokio::time::pause();

    let response = execute_operation(
        &harness,
        ProtocolOperation::ResponsesCompact,
        "compact-model",
        json!({"input":[]}),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.json_body()["output"][0]["type"], "compaction");
    assert_eq!(
        transport.calls()[0].read_timeout,
        Duration::from_secs(1_200)
    );
    tokio::time::resume();
    harness.telemetry.shutdown(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn claude_json_and_sse_persist_exact_cumulative_usage() {
    let transport = Arc::new(ScriptedTransport::new([
        ScriptStep::json(
            StatusCode::OK,
            r#"{"id":"msg-json","model":"upstream","content":[],"usage":{"input_tokens":60,"output_tokens":8,"cache_read_input_tokens":12,"cache_creation_input_tokens":3}}"#,
        ),
        ScriptStep::stream(
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg-stream\",\"model\":\"upstream\",\"usage\":{\"input_tokens\":55,\"output_tokens\":1,\"cache_read_input_tokens\":11,\"cache_creation_input_tokens\":4}}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"hello\"}}\n\nevent: message_delta\ndata: {\"type\":\"message_delta\",\"usage\":{\"output_tokens\":9}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
        ),
    ]));
    let harness = harness_for_protocol(
        transport,
        1,
        &["claude-usage-model"],
        &[],
        ProviderKind::Claude,
        ProtocolDialect::AnthropicMessages,
        None,
    )
    .await;

    let json_response = execute_operation(
        &harness,
        ProtocolOperation::Messages,
        "claude-usage-model",
        json!({"messages":[]}),
    )
    .await;
    assert_eq!(json_response.status(), StatusCode::OK);
    let json_record = wait_for_log(&harness, json_response.request_id).await;
    assert_eq!(json_record.request.first_token_ms, None);
    assert_eq!(json_record.request.input_tokens, Some(60));
    assert_eq!(json_record.request.output_tokens, Some(8));
    assert_eq!(json_record.request.cache_read_tokens, Some(12));

    let stream_request_id = RequestId::new();
    let stream_response = execute_stream_operation(
        &harness,
        stream_request_id,
        ProtocolOperation::Messages,
        "claude-usage-model",
        json!({"messages":[]}),
    )
    .await;
    let mut body = streaming_body(stream_response);
    while let Some(frame) = body.next().await {
        frame.expect("valid Claude SSE frame");
    }
    let stream_record = wait_for_log(&harness, stream_request_id).await;
    assert!(stream_record.request.first_token_ms.is_some());
    assert_eq!(stream_record.request.input_tokens, Some(55));
    assert_eq!(stream_record.request.output_tokens, Some(9));
    assert_eq!(stream_record.request.cache_read_tokens, Some(11));
    harness.telemetry.shutdown(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn count_tokens_result_is_not_recorded_as_generation_usage() {
    let transport = Arc::new(ScriptedTransport::new([ScriptStep::json(
        StatusCode::OK,
        r#"{"input_tokens":37,"usage":{"input_tokens":999,"output_tokens":888,"cache_read_input_tokens":777,"cache_creation_input_tokens":666}}"#,
    )]));
    let harness = harness_for_protocol(
        transport,
        1,
        &["count-usage-model"],
        &[],
        ProviderKind::Claude,
        ProtocolDialect::AnthropicMessages,
        None,
    )
    .await;

    let response = execute_operation(
        &harness,
        ProtocolOperation::MessagesCountTokens,
        "count-usage-model",
        json!({"messages":[]}),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let record = wait_for_log(&harness, response.request_id).await;
    assert_eq!(record.request.first_token_ms, None);
    assert_eq!(record.request.input_tokens, None);
    assert_eq!(record.request.output_tokens, None);
    assert_eq!(record.request.cache_read_tokens, None);
    harness.telemetry.shutdown(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn ambiguous_transport_failure_is_not_retried() {
    let transport = Arc::new(ScriptedTransport::new([
        ScriptStep::Error(TransportError::new(
            TransportErrorStage::AwaitHeaders,
            TransportFailureScope::Endpoint,
            RetrySafety::Ambiguous,
            "response lost",
        )),
        ScriptStep::json(
            StatusCode::OK,
            r#"{"id":"must-not-run","model":"upstream","output":[]}"#,
        ),
    ]));
    let harness = harness(transport.clone(), 2, &["ambiguous-model"], &[]).await;

    let response = execute_json(&harness, "ambiguous-model", json!({"input":"hello"})).await;

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    assert_eq!(response.json_body()["error"]["code"], "upstream_error");
    assert_eq!(transport.calls().len(), 1);
}

#[tokio::test]
async fn buffered_body_read_timeout_is_ambiguous_and_not_retried() {
    let transport = Arc::new(ScriptedTransport::new([
        ScriptStep::stalled_json(StatusCode::OK, r#"{"id":"partial","model":"upstream"}"#),
        ScriptStep::json(
            StatusCode::OK,
            r#"{"id":"must-not-run","model":"upstream","output":[]}"#,
        ),
    ]));
    let harness = harness(
        transport.clone(),
        2,
        &["body-timeout-model"],
        &[(
            SettingKey::UpstreamReadTimeout,
            SettingValue::DurationSecs(1),
        )],
    )
    .await;

    let response = execute_json(&harness, "body-timeout-model", json!({"input":"hello"})).await;

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    assert_eq!(response.json_body()["error"]["code"], "upstream_error");
    assert_eq!(transport.calls().len(), 1);
}

#[tokio::test]
async fn rate_limit_returns_retry_after_and_cools_only_that_model() {
    const UPSTREAM_BODY: &str =
        r#"{"error":{"type":"rate_limit_error","message":"Official rate limit detail"}}"#;
    let mut retry_after = HeaderMap::new();
    retry_after.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/problem+json"),
    );
    retry_after.insert(header::RETRY_AFTER, HeaderValue::from_static("5"));
    retry_after.insert(
        "x-request-id",
        HeaderValue::from_static("rate-limit-request"),
    );
    retry_after.insert(
        "x-codex-primary-used-percent",
        HeaderValue::from_static("100"),
    );
    let transport = Arc::new(ScriptedTransport::new([
        ScriptStep::json_with_headers(StatusCode::TOO_MANY_REQUESTS, retry_after, UPSTREAM_BODY),
        ScriptStep::json(
            StatusCode::OK,
            r#"{"id":"other-ok","model":"upstream","output":[]}"#,
        ),
    ]));
    let harness = harness(transport.clone(), 1, &["limited-model", "other-model"], &[]).await;

    let limited = execute_json(&harness, "limited-model", json!({"input":"one"})).await;
    assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(limited.body_bytes(), UPSTREAM_BODY.as_bytes());
    assert_eq!(
        limited.headers()[header::CONTENT_TYPE],
        "application/problem+json"
    );
    assert_eq!(limited.headers()[header::RETRY_AFTER], "5");
    assert_eq!(limited.headers()["x-request-id"], "rate-limit-request");
    assert_eq!(limited.headers()["x-codex-primary-used-percent"], "100");
    let record = wait_for_log(&harness, limited.request_id).await;
    assert_eq!(
        record.request.error_message.as_deref(),
        Some("Official rate limit detail")
    );
    assert_eq!(
        record.attempts[0].error_message.as_deref(),
        Some("Official rate limit detail")
    );

    let limited_again = execute_json(&harness, "limited-model", json!({"input":"two"})).await;
    assert_eq!(limited_again.status(), StatusCode::TOO_MANY_REQUESTS);
    let retry_seconds = limited_again.headers()[header::RETRY_AFTER]
        .to_str()
        .expect("retry-after header")
        .parse::<u64>()
        .expect("retry-after seconds");
    assert!((1..=5).contains(&retry_seconds));
    assert_eq!(transport.calls().len(), 1);

    let other = execute_json(&harness, "other-model", json!({"input":"three"})).await;
    assert_eq!(other.status(), StatusCode::OK);
    assert_eq!(transport.calls().len(), 2);
}

#[tokio::test]
async fn upstream_error_status_headers_and_body_are_forwarded_verbatim() {
    const CASES: &[(u16, &str, &str)] = &[
        (
            401,
            "application/json",
            r#"{"error":{"type":"authentication_error","message":"upstream 401"}}"#,
        ),
        (
            404,
            "application/problem+json",
            r#"{"error":{"code":"model_not_found","message":"upstream 404"}}"#,
        ),
        (429, "text/plain; charset=utf-8", "upstream 429 plain text"),
        (500, "text/plain", "upstream 500 plain text"),
        (
            529,
            "application/json",
            r#"{"error":{"type":"overloaded_error","message":"upstream 529"}}"#,
        ),
        (598, "text/plain", "unknown upstream status 598"),
    ];

    for &(status, content_type, upstream_body) in CASES {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_str(content_type).expect("content type"),
        );
        headers.insert(
            "x-request-id",
            HeaderValue::from_str(&format!("status-{status}")).expect("request id"),
        );
        let transport = Arc::new(ScriptedTransport::new([ScriptStep::json_with_headers(
            StatusCode::from_u16(status).expect("upstream status"),
            headers,
            upstream_body,
        )]));
        let model = format!("status-{status}-model");
        let harness = harness(transport, 1, &[model.as_str()], &[]).await;

        let response = execute_json(&harness, &model, json!({"input":"one"})).await;

        assert_eq!(response.status().as_u16(), status);
        assert_eq!(response.body_bytes(), upstream_body.as_bytes());
        assert_eq!(response.headers()[header::CONTENT_TYPE], content_type);
        assert_eq!(
            response.headers()["x-request-id"],
            format!("status-{status}")
        );
        harness.telemetry.shutdown(Duration::from_secs(1)).await;
    }
}

#[tokio::test]
async fn encoded_upstream_error_keeps_content_encoding_and_body_verbatim() {
    const UPSTREAM_BODY: &[u8] = b"\x1f\x8b\x08\x00encoded-upstream-error";
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
    headers.insert(
        "x-request-id",
        HeaderValue::from_static("encoded-error-request"),
    );
    let transport = Arc::new(ScriptedTransport::new([ScriptStep::Json {
        status: StatusCode::BAD_REQUEST,
        headers,
        body: Bytes::from_static(UPSTREAM_BODY),
    }]));
    let harness = harness(transport, 1, &["encoded-error-model"], &[]).await;

    let response = execute_json(&harness, "encoded-error-model", json!({"input":"hello"})).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.body_bytes(), UPSTREAM_BODY);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
    assert_eq!(response.headers()[header::CONTENT_ENCODING], "gzip");
    assert_eq!(response.headers()["x-request-id"], "encoded-error-request");
}

#[tokio::test]
async fn final_openai_overload_keeps_529_body_and_safe_headers() {
    const UPSTREAM_BODY: &str =
        r#"{"error":{"type":"overloaded_error","message":"Official overload detail"}}"#;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(header::RETRY_AFTER, HeaderValue::from_static("2"));
    headers.insert("x-request-id", HeaderValue::from_static("overload-request"));
    let transport = Arc::new(ScriptedTransport::new([ScriptStep::json_with_headers(
        StatusCode::from_u16(529).expect("529 status"),
        headers,
        UPSTREAM_BODY,
    )]));
    let harness = harness(transport, 1, &["overload-model"], &[]).await;

    let response = execute_json(&harness, "overload-model", json!({"input":"one"})).await;

    assert_eq!(response.status().as_u16(), 529);
    assert_eq!(response.body_bytes(), UPSTREAM_BODY.as_bytes());
    assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
    assert_eq!(response.headers()[header::RETRY_AFTER], "2");
    assert_eq!(response.headers()["x-request-id"], "overload-request");
}

#[tokio::test]
async fn oversized_error_body_keeps_529_headers_and_returns_an_empty_body() {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(header::RETRY_AFTER, HeaderValue::from_static("4"));
    headers.insert(
        "x-request-id",
        HeaderValue::from_static("oversized-overload-request"),
    );
    let body = Bytes::from(format!(
        r#"{{"error":{{"type":"overloaded_error","message":"discarded"}},"padding":"{}"}}"#,
        "x".repeat(70 * 1024)
    ));
    let transport = Arc::new(ScriptedTransport::new([ScriptStep::Json {
        status: StatusCode::from_u16(529).expect("529 status"),
        headers,
        body,
    }]));
    let harness = harness(transport, 1, &["oversized-overload-model"], &[]).await;

    let response = execute_json(
        &harness,
        "oversized-overload-model",
        json!({"input":"hello"}),
    )
    .await;

    assert_eq!(response.status().as_u16(), 529);
    assert!(response.body_bytes().is_empty());
    assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
    assert_eq!(response.headers()[header::RETRY_AFTER], "4");
    assert_eq!(
        response.headers()["x-request-id"],
        "oversized-overload-request"
    );
    let record = wait_for_log(&harness, response.request_id).await;
    assert_eq!(record.request.error_class, Some(ErrorClass::Upstream));
    assert_eq!(record.request.error_message, None);
    harness.telemetry.shutdown(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn incomplete_error_body_keeps_429_headers_and_returns_an_empty_body() {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/problem+json"),
    );
    headers.insert(header::RETRY_AFTER, HeaderValue::from_static("3"));
    headers.insert(
        "x-request-id",
        HeaderValue::from_static("incomplete-rate-limit-request"),
    );
    let transport = Arc::new(ScriptedTransport::new([
        ScriptStep::stalled_json_with_headers(
            StatusCode::TOO_MANY_REQUESTS,
            headers,
            r#"{"error":{"type":"rate_limit_error","message":"must not escape an incomplete error body"}}"#,
        ),
    ]));
    let harness = harness(
        transport,
        1,
        &["incomplete-rate-limit-model"],
        &[(
            SettingKey::UpstreamReadTimeout,
            SettingValue::DurationSecs(1),
        )],
    )
    .await;

    let response = execute_json(
        &harness,
        "incomplete-rate-limit-model",
        json!({"input":"hello"}),
    )
    .await;

    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(response.body_bytes().is_empty());
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "application/problem+json"
    );
    assert_eq!(response.headers()[header::RETRY_AFTER], "3");
    assert_eq!(
        response.headers()["x-request-id"],
        "incomplete-rate-limit-request"
    );
}

#[tokio::test]
async fn buffered_error_body_deadline_keeps_upstream_status_and_headers() {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/problem+json"),
    );
    headers.insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
    headers.insert(header::RETRY_AFTER, HeaderValue::from_static("7"));
    headers.insert(
        "x-request-id",
        HeaderValue::from_static("buffered-deadline-request"),
    );
    let transport = Arc::new(ScriptedTransport::new([
        ScriptStep::stalled_json_with_headers(
            StatusCode::TOO_MANY_REQUESTS,
            headers,
            r#"{"error":{"message":"partial"}}"#,
        ),
    ]));
    let harness = harness(
        transport,
        1,
        &["buffered-deadline-model"],
        &[(
            SettingKey::UpstreamReadTimeout,
            SettingValue::DurationSecs(10),
        )],
    )
    .await;
    tokio::time::pause();

    let response = execute_json(
        &harness,
        "buffered-deadline-model",
        json!({"input":"hello"}),
    )
    .await;
    tokio::time::resume();

    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(response.body_bytes().is_empty());
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "application/problem+json"
    );
    assert!(!response.headers().contains_key(header::CONTENT_ENCODING));
    assert_eq!(response.headers()[header::RETRY_AFTER], "7");
    assert_eq!(
        response.headers()["x-request-id"],
        "buffered-deadline-request"
    );
}

#[tokio::test]
async fn stream_request_error_body_deadline_keeps_upstream_status_and_headers() {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/problem+json"),
    );
    headers.insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
    headers.insert(header::RETRY_AFTER, HeaderValue::from_static("9"));
    headers.insert(
        "x-request-id",
        HeaderValue::from_static("stream-deadline-request"),
    );
    let transport = Arc::new(ScriptedTransport::new([
        ScriptStep::stalled_json_with_headers(
            StatusCode::TOO_MANY_REQUESTS,
            headers,
            r#"{"error":{"message":"partial"}}"#,
        ),
    ]));
    let harness = harness(
        transport,
        1,
        &["stream-deadline-model"],
        &[(
            SettingKey::UpstreamReadTimeout,
            SettingValue::DurationSecs(10),
        )],
    )
    .await;
    tokio::time::pause();

    let response = execute_json(
        &harness,
        "stream-deadline-model",
        json!({"input":"hello","stream":true}),
    )
    .await;
    tokio::time::resume();

    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(response.body_bytes().is_empty());
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "application/problem+json"
    );
    assert!(!response.headers().contains_key(header::CONTENT_ENCODING));
    assert_eq!(response.headers()[header::RETRY_AFTER], "9");
    assert_eq!(
        response.headers()["x-request-id"],
        "stream-deadline-request"
    );
}

#[tokio::test]
async fn retried_attempt_headers_are_discarded_in_favor_of_the_committed_attempt() {
    let mut first_headers = HeaderMap::new();
    first_headers.insert(
        "x-request-id",
        HeaderValue::from_static("discarded-request"),
    );
    first_headers.insert(
        "x-codex-promo-message",
        HeaderValue::from_static("discarded-promo"),
    );
    let mut second_headers = HeaderMap::new();
    second_headers.insert(
        "x-request-id",
        HeaderValue::from_static("committed-request"),
    );
    let transport = Arc::new(ScriptedTransport::new([
        ScriptStep::json_with_headers(
            StatusCode::TOO_MANY_REQUESTS,
            first_headers,
            r#"{"error":{"type":"rate_limit_error"}}"#,
        ),
        ScriptStep::json_with_headers(
            StatusCode::OK,
            second_headers,
            r#"{"id":"ok","model":"upstream","output":[]}"#,
        ),
    ]));
    let harness = harness(transport.clone(), 2, &["retry-header-model"], &[]).await;

    let response = execute_json(&harness, "retry-header-model", json!({"input":"hello"})).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["x-request-id"], "committed-request");
    assert!(!response.headers().contains_key("x-codex-promo-message"));
    assert_eq!(transport.calls().len(), 2);
}

#[tokio::test]
async fn only_the_final_attempt_status_headers_and_body_reach_the_client() {
    const FIRST_BODY: &str =
        r#"{"error":{"type":"rate_limit_error","message":"discarded upstream detail"}}"#;
    const FINAL_BODY: &str =
        r#"{"error":{"type":"authentication_error","message":"final upstream detail"}}"#;
    let mut first_headers = HeaderMap::new();
    first_headers.insert(
        "x-request-id",
        HeaderValue::from_static("discarded-request"),
    );
    let mut final_headers = HeaderMap::new();
    final_headers.insert("x-request-id", HeaderValue::from_static("final-request"));
    let transport = Arc::new(ScriptedTransport::new([
        ScriptStep::json_with_headers(StatusCode::TOO_MANY_REQUESTS, first_headers, FIRST_BODY),
        ScriptStep::json_with_headers(StatusCode::UNAUTHORIZED, final_headers, FINAL_BODY),
    ]));
    let harness = harness(transport.clone(), 2, &["final-message-model"], &[]).await;

    let response = execute_json(&harness, "final-message-model", json!({"input":"hello"})).await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(response.body_bytes(), FINAL_BODY.as_bytes());
    assert_eq!(response.headers()["x-request-id"], "final-request");
    assert_eq!(transport.calls().len(), 2);
    let record = wait_for_log(&harness, response.request_id).await;
    assert_eq!(record.attempts.len(), 2);
    assert_eq!(
        record.attempts[0].error_message.as_deref(),
        Some("discarded upstream detail")
    );
    assert_eq!(
        record.attempts[1].error_message.as_deref(),
        Some("final upstream detail")
    );
    assert_eq!(
        record.request.error_message.as_deref(),
        Some("final upstream detail")
    );
    harness.telemetry.shutdown(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn grok_non_success_stream_request_returns_and_logs_official_message() {
    const UPSTREAM_BODY: &str = r#"{"error":{"code":"subscription:free-usage-exhausted","message":"tokens (actual/limit): 1065387/1000000; Usage resets over a rolling 24-hour window"}}"#;
    const OFFICIAL_MESSAGE: &str =
        "tokens (actual/limit): 1065387/1000000; Usage resets over a rolling 24-hour window";
    let transport = Arc::new(ScriptedTransport::new([ScriptStep::json(
        StatusCode::TOO_MANY_REQUESTS,
        UPSTREAM_BODY,
    )]));
    let harness = harness_for_protocol(
        transport,
        1,
        &["grok-stream-error-model"],
        &[],
        ProviderKind::Grok,
        ProtocolDialect::OpenAiResponses,
        None,
    )
    .await;

    let response = execute_json(
        &harness,
        "grok-stream-error-model",
        json!({"input":"hello","stream":true}),
    )
    .await;

    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(response.body_bytes(), UPSTREAM_BODY.as_bytes());
    let record = wait_for_log(&harness, response.request_id).await;
    assert_eq!(record.request.error_class, Some(ErrorClass::QuotaExhausted));
    assert_eq!(
        record.request.error_message.as_deref(),
        Some(OFFICIAL_MESSAGE)
    );
    assert!(
        record
            .attempts
            .iter()
            .filter_map(|attempt| attempt.error_message.as_deref())
            .all(|message| message == OFFICIAL_MESSAGE)
    );
    harness.telemetry.shutdown(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn upstream_credential_unauthorized_is_forwarded_verbatim() {
    const UPSTREAM_BODY: &str =
        r#"{"error":{"type":"authentication_error","message":"Official authentication detail"}}"#;
    let transport = Arc::new(ScriptedTransport::new([ScriptStep::json(
        StatusCode::UNAUTHORIZED,
        UPSTREAM_BODY,
    )]));
    let harness = harness(transport, 1, &["auth-model"], &[]).await;

    let response = execute_json(&harness, "auth-model", json!({"input":"one"})).await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(response.body_bytes(), UPSTREAM_BODY.as_bytes());
}

#[tokio::test]
async fn cross_protocol_upstream_error_is_not_reencoded() {
    const UPSTREAM_BODY: &str = "chat upstream says model missing";
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    let transport = Arc::new(ScriptedTransport::new([ScriptStep::json_with_headers(
        StatusCode::NOT_FOUND,
        headers,
        UPSTREAM_BODY,
    )]));
    let harness = harness_for_protocol(
        transport.clone(),
        1,
        &["cross-protocol-error-model"],
        &[],
        ProviderKind::Codex,
        ProtocolDialect::OpenAiResponses,
        Some(ProtocolDialect::OpenAiChatCompletions),
    )
    .await;

    let response = execute_json(
        &harness,
        "cross-protocol-error-model",
        json!({"input":"one"}),
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(response.body_bytes(), UPSTREAM_BODY.as_bytes());
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/plain; charset=utf-8"
    );
    assert!(transport.calls()[0].uri.ends_with("/chat/completions"));
    harness.telemetry.shutdown(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn continuation_binding_failure_never_switches_credentials() {
    let transport = Arc::new(ScriptedTransport::new([
        ScriptStep::json(
            StatusCode::OK,
            r#"{"id":"bound-id","model":"upstream","output":[]}"#,
        ),
        ScriptStep::Error(TransportError::new(
            TransportErrorStage::Tcp,
            TransportFailureScope::Endpoint,
            RetrySafety::DefinitelyNotSent,
            "bound target unavailable",
        )),
        ScriptStep::json(
            StatusCode::OK,
            r#"{"id":"wrong-target","model":"upstream","output":[]}"#,
        ),
    ]));
    let harness = harness(transport.clone(), 2, &["bound-model"], &[]).await;

    let first = execute_json(&harness, "bound-model", json!({"input":"start"})).await;
    assert_eq!(first.status(), StatusCode::OK);
    let first_auth = transport.calls()[0].authorization.clone();

    let second = execute_json(
        &harness,
        "bound-model",
        json!({"previous_response_id":"bound-id","input":"continue"}),
    )
    .await;

    assert_eq!(second.status(), StatusCode::BAD_GATEWAY);
    let calls = transport.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[1].authorization, first_auth);
}

#[tokio::test]
async fn total_attempt_budget_stops_before_a_fourth_attempt() {
    let transport = Arc::new(ScriptedTransport::new([
        failure_step(),
        failure_step(),
        failure_step(),
        ScriptStep::json(
            StatusCode::OK,
            r#"{"id":"must-not-run","model":"upstream","output":[]}"#,
        ),
    ]));
    let harness = harness(transport.clone(), 4, &["budget-model"], &[]).await;

    let response = execute_json(&harness, "budget-model", json!({"input":"hello"})).await;

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    assert_eq!(response.json_body()["error"]["code"], "upstream_error");
    assert_eq!(transport.calls().len(), 3);
    let record = wait_for_log(&harness, response.request_id).await;
    assert_eq!(record.request.attempt_count, 3);
    assert_eq!(record.request.error_class, Some(ErrorClass::Network));
    assert_eq!(record.attempts.len(), 3);
    assert!(
        record
            .attempts
            .iter()
            .all(|attempt| attempt.outcome == RequestAttemptOutcome::TransportError)
    );
    harness.telemetry.shutdown(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn credential_switch_budget_stops_before_switching_again() {
    let transport = Arc::new(ScriptedTransport::new([
        failure_step(),
        failure_step(),
        ScriptStep::json(
            StatusCode::OK,
            r#"{"id":"must-not-run","model":"upstream","output":[]}"#,
        ),
    ]));
    let harness = harness(
        transport.clone(),
        3,
        &["switch-model"],
        &[(
            SettingKey::RetryMaxCredentialSwitches,
            SettingValue::Integer(1),
        )],
    )
    .await;

    let response = execute_json(&harness, "switch-model", json!({"input":"hello"})).await;

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    assert_eq!(response.json_body()["error"]["code"], "upstream_error");
    assert_eq!(transport.calls().len(), 2);
}

#[tokio::test]
async fn sse_first_frame_failure_does_not_start_a_second_stream() {
    let transport = Arc::new(ScriptedTransport::new([
        ScriptStep::stream_error(TransportError::new(
            TransportErrorStage::ReadBody,
            TransportFailureScope::Endpoint,
            RetrySafety::Ambiguous,
            "stream ended before first event",
        )),
        ScriptStep::stream(
            r#"event: response.created\ndata: {\"response\":{\"id\":\"wrong-stream\"}}\n\n"#,
        ),
    ]));
    let harness = harness(transport.clone(), 2, &["stream-model"], &[]).await;

    let response = execute_json(
        &harness,
        "stream-model",
        json!({"input":"hello","stream":true}),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    assert_eq!(transport.calls().len(), 1);
}

#[tokio::test]
async fn sse_postcommit_idle_timeout_does_not_start_a_second_stream() {
    let transport = Arc::new(ScriptedTransport::new([
        ScriptStep::stalled_stream(
            "event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"postcommit-id\",\"model\":\"upstream\"}}\n\n",
        ),
        ScriptStep::stream(
            "event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"must-not-run\",\"model\":\"upstream\"}}\n\n",
        ),
    ]));
    let harness = harness(
        transport.clone(),
        2,
        &["postcommit-model"],
        &[(
            SettingKey::StreamPostcommitIdleTimeout,
            SettingValue::DurationSecs(1),
        )],
    )
    .await;
    let request_id = RequestId::new();
    let response = harness
        .service
        .execute(
            Arc::clone(&harness.snapshot),
            PublicRequest {
                request_id,
                gateway_api_key_id: GatewayApiKeyId::new(),
                client_ip: "127.0.0.1".parse().expect("client IP"),
                operation: ProtocolOperation::Responses,
                headers: HeaderMap::new(),
                body: Bytes::from_static(
                    br#"{"model":"postcommit-model","input":"hello","stream":true}"#,
                ),
            },
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
    let mut body = match response.body {
        PublicResponseBody::Streaming(body) => body,
        PublicResponseBody::Buffered(_) => panic!("test expected streaming response"),
    };
    assert!(body.next().await.expect("first frame").is_ok());
    assert!(body.next().await.expect("idle timeout error").is_err());
    assert_eq!(transport.calls().len(), 1);
    let record = wait_for_log(&harness, request_id).await;
    assert!(record.request.is_stream);
    assert_eq!(record.request.status_code, 200);
    assert_eq!(record.request.error_class, Some(ErrorClass::Network));
    assert_eq!(record.attempts.len(), 1);
    assert_eq!(
        record.attempts[0].outcome,
        RequestAttemptOutcome::StreamError
    );
    assert_eq!(
        record.attempts[0].retry_safety,
        Some(RetrySafety::Ambiguous)
    );
    assert_eq!(record.attempts[0].error_class, Some(ErrorClass::Network));
    assert_eq!(record.attempts[0].status_code, Some(200));
    harness.telemetry.shutdown(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn responses_eof_before_terminal_persists_a_stream_error() {
    let transport = Arc::new(ScriptedTransport::new([ScriptStep::stream(
        "event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"eof-id\",\"model\":\"upstream\"}}\n\n",
    )]));
    let harness = harness(transport, 1, &["eof-model"], &[]).await;
    let request_id = RequestId::new();
    let response = execute_stream(&harness, request_id, "eof-model").await;
    let mut body = streaming_body(response);

    assert!(body.next().await.expect("created frame").is_ok());
    let error = body
        .next()
        .await
        .expect("missing terminal body error")
        .expect_err("EOF without terminal must fail");
    assert!(error.to_string().contains("terminal event"));
    assert!(body.next().await.is_none());

    let record = wait_for_log(&harness, request_id).await;
    assert!(record.request.is_stream);
    assert_eq!(record.request.status_code, 200);
    assert_eq!(record.request.error_class, Some(ErrorClass::Upstream));
    assert_eq!(record.request.first_token_ms, None);
    assert_eq!(record.request.input_tokens, None);
    assert_eq!(record.request.output_tokens, None);
    assert_eq!(record.attempts.len(), 1);
    assert_eq!(
        record.attempts[0].outcome,
        RequestAttemptOutcome::StreamError
    );
    assert_eq!(
        record.attempts[0].retry_safety,
        Some(RetrySafety::Ambiguous)
    );
    assert_eq!(record.attempts[0].error_class, Some(ErrorClass::Upstream));
    assert_eq!(record.attempts[0].status_code, Some(200));
    harness.telemetry.shutdown(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn responses_failed_terminal_is_delivered_and_persists_a_stream_error() {
    let transport = Arc::new(ScriptedTransport::new([ScriptStep::stream(
        "event: response.failed\ndata: {\"type\":\"response.failed\",\"response\":{\"id\":\"failed-id\",\"model\":\"upstream\",\"error\":{\"code\":\"server_error\"}}}\n\n",
    )]));
    let harness = harness(transport, 1, &["failed-model"], &[]).await;
    let request_id = RequestId::new();
    let response = execute_stream(&harness, request_id, "failed-model").await;
    let mut body = streaming_body(response);

    let frame = body
        .next()
        .await
        .expect("failed terminal frame")
        .expect("failed terminal bytes");
    assert!(String::from_utf8_lossy(&frame).contains("response.failed"));
    assert!(body.next().await.is_none());

    let record = wait_for_log(&harness, request_id).await;
    assert_eq!(record.request.status_code, 200);
    assert_eq!(record.request.error_class, Some(ErrorClass::Upstream));
    assert_eq!(record.attempts.len(), 1);
    assert_eq!(
        record.attempts[0].outcome,
        RequestAttemptOutcome::StreamError
    );
    assert_eq!(
        record.attempts[0].retry_safety,
        Some(RetrySafety::Ambiguous)
    );
    assert_eq!(record.attempts[0].error_class, Some(ErrorClass::Upstream));
    assert_eq!(record.attempts[0].status_code, Some(200));
    harness.telemetry.shutdown(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn sse_persists_client_visible_ttft_and_terminal_usage() {
    let transport = Arc::new(ScriptedTransport::new([ScriptStep::stream(
        "event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"usage-stream-id\",\"model\":\"upstream\"}}\n\nevent: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"hello\"}\n\nevent: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":80,\"output_tokens\":12,\"input_tokens_details\":{\"cached_tokens\":20,\"cache_write_tokens\":4}}}}\n\n",
    )]));
    let harness = harness(transport, 1, &["usage-stream-model"], &[]).await;
    let request_id = RequestId::new();
    let response = execute_stream(&harness, request_id, "usage-stream-model").await;
    let mut body = streaming_body(response);

    while let Some(frame) = body.next().await {
        frame.expect("valid SSE frame");
    }

    let record = wait_for_log(&harness, request_id).await;
    assert!(record.request.first_token_ms.is_some());
    assert_eq!(record.request.input_tokens, Some(80));
    assert_eq!(record.request.output_tokens, Some(12));
    assert_eq!(record.request.cache_read_tokens, Some(20));
    harness.telemetry.shutdown(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn sse_client_drop_persists_cancellation_once() {
    let transport = Arc::new(ScriptedTransport::new([ScriptStep::stalled_stream(
        "event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"drop-id\",\"model\":\"upstream\"}}\n\n",
    )]));
    let harness = harness(transport, 1, &["drop-model"], &[]).await;
    let request_id = RequestId::new();
    let response = execute_stream(&harness, request_id, "drop-model").await;
    let mut body = streaming_body(response);
    assert!(body.next().await.expect("first frame").is_ok());
    drop(body);

    let record = wait_for_log(&harness, request_id).await;
    assert_eq!(record.request.status_code, 200);
    assert_eq!(record.request.error_class, Some(ErrorClass::Cancelled));
    assert_eq!(record.request.attempt_count, 1);
    assert_eq!(record.attempts.len(), 1);
    assert_eq!(record.attempts[0].outcome, RequestAttemptOutcome::Cancelled);
    assert_eq!(record.attempts[0].error_class, Some(ErrorClass::Cancelled));
    assert_eq!(record.attempts[0].status_code, Some(200));
    harness.telemetry.shutdown(Duration::from_secs(1)).await;
}

#[tokio::test]
async fn primed_content_is_not_ttft_until_the_client_polls_it() {
    let transport = Arc::new(ScriptedTransport::new([ScriptStep::stream(
        "event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"buffered\"}\n\n",
    )]));
    let harness = harness(transport, 1, &["primed-content-model"], &[]).await;
    let request_id = RequestId::new();
    let response = execute_stream(&harness, request_id, "primed-content-model").await;
    let body = streaming_body(response);

    drop(body);

    let record = wait_for_log(&harness, request_id).await;
    assert_eq!(record.request.first_token_ms, None);
    assert_eq!(record.request.error_class, Some(ErrorClass::Cancelled));
    harness.telemetry.shutdown(Duration::from_secs(1)).await;
}

fn failure_step() -> ScriptStep {
    ScriptStep::Error(TransportError::new(
        TransportErrorStage::Tcp,
        TransportFailureScope::Endpoint,
        RetrySafety::DefinitelyNotSent,
        "test connection failure",
    ))
}

struct Harness {
    _directory: TempDir,
    snapshot: Arc<PublishedSnapshot>,
    service: Arc<PublicRequestService>,
    telemetry: Arc<RequestTelemetry>,
}

async fn harness(
    transport: Arc<ScriptedTransport>,
    endpoint_count: usize,
    models: &[&str],
    overrides: &[(SettingKey, SettingValue)],
) -> Harness {
    harness_for_protocol(
        transport,
        endpoint_count,
        models,
        overrides,
        ProviderKind::Codex,
        ProtocolDialect::OpenAiResponses,
        None,
    )
    .await
}

async fn harness_for_protocol(
    transport: Arc<ScriptedTransport>,
    endpoint_count: usize,
    models: &[&str],
    overrides: &[(SettingKey, SettingValue)],
    provider_kind: ProviderKind,
    protocol_dialect: ProtocolDialect,
    upstream_protocol_dialect: Option<ProtocolDialect>,
) -> Harness {
    let directory = tempfile::tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("config.sqlite3"))
            .await
            .expect("storage"),
    );
    let configuration = storage.load_configuration().await.expect("configuration");
    let runtime = Arc::new(RuntimeRegistry::new());
    let telemetry = Arc::new(RequestTelemetry::start(
        Arc::clone(&storage),
        configuration.revision(),
        configuration.settings().logging(),
        &runtime.lifecycle(),
    ));
    let snapshots = Arc::new(SnapshotStore::new(
        PublishedSnapshot::new(
            configuration,
            runtime.as_ref(),
            any2api_contract_tests::build_provider_registry().as_ref(),
        )
        .expect("initial snapshot"),
    ));
    let publisher = ConfigPublisher::new(
        Arc::clone(&storage),
        Arc::clone(&snapshots),
        Arc::clone(&runtime),
        any2api_contract_tests::build_configuration_capabilities(),
    )
    .expect("configuration publisher");
    let mut published = snapshots.load();
    for (key, value) in default_overrides()
        .into_iter()
        .chain(overrides.iter().cloned())
    {
        published = publisher
            .set_setting_override(published.revision(), key, value)
            .await
            .expect("setting override");
    }

    let selected_models = models
        .iter()
        .map(|model| (*model).to_owned())
        .collect::<Vec<_>>();
    for index in 0..endpoint_count {
        let endpoint_id = ProviderEndpointId::new();
        let endpoint = publisher
            .create_provider_endpoint(
                published.revision(),
                endpoint_id,
                ProviderEndpointDraft::with_upstream_protocol(
                    format!("Endpoint {index}"),
                    provider_kind,
                    format!("https://upstream-{index}.example.com/v1"),
                    protocol_dialect,
                    upstream_protocol_dialect,
                    true,
                )
                .expect("endpoint draft"),
            )
            .await
            .expect("endpoint publish");
        let credential_id = CredentialId::new();
        published = publisher
            .create_provider_credential(
                endpoint.revision(),
                credential_id,
                endpoint_id,
                ProviderCredentialDraft::new(
                    format!("Credential {index}"),
                    CredentialKind::ApiKey,
                    ProxyProfileId::DIRECT,
                    None,
                    true,
                )
                .expect("credential draft"),
                ProviderApiKeySecret::new(format!("sk-reliability-{index}")),
            )
            .await
            .expect("credential publish");
        published = publisher
            .set_provider_credential_models(
                published.revision(),
                credential_id,
                1,
                selected_models.clone(),
            )
            .await
            .expect("credential model publish");
    }

    let mut protocols = ProtocolRegistry::new();
    protocols
        .register(Arc::new(OpenAiResponsesAdapter::new()))
        .expect("responses adapter");
    protocols
        .register(Arc::new(OpenAiChatCompletionsAdapter::new()))
        .expect("chat completions adapter");
    protocols
        .register(Arc::new(OpenAiImagesAdapter::new()))
        .expect("images adapter");
    protocols
        .register(Arc::new(AnthropicMessagesAdapter::new()))
        .expect("messages adapter");
    protocols
        .register_bridge(Arc::new(ResponsesToChatCompletionsBridge::new()))
        .expect("Responses to Chat Completions bridge");
    let providers = any2api_contract_tests::build_provider_registry();
    let transport_manager: Arc<dyn TransportManager> = transport;
    let service = Arc::new(
        PublicRequestService::new(Arc::new(protocols), providers, transport_manager)
            .expect("public request service")
            .with_telemetry(Arc::clone(&telemetry)),
    );
    Harness {
        _directory: directory,
        snapshot: published,
        service,
        telemetry,
    }
}

fn default_overrides() -> Vec<(SettingKey, SettingValue)> {
    vec![
        (
            SettingKey::SchedulerOnRateLimited,
            SettingValue::RateLimitMode(RateLimitMode::Reject),
        ),
        (SettingKey::RetryBaseDelay, SettingValue::DurationSecs(0)),
        (SettingKey::RetryMaxDelay, SettingValue::DurationSecs(0)),
        (SettingKey::RetryJitterRatio, SettingValue::Integer(0)),
        (
            SettingKey::AffinityWaitTimeout,
            SettingValue::DurationSecs(1),
        ),
        (
            SettingKey::RetryPrecommitTotalBudget,
            SettingValue::DurationSecs(1),
        ),
    ]
}

async fn execute_json(harness: &Harness, model: &str, extra: Value) -> TestResponse {
    execute_operation(harness, ProtocolOperation::Responses, model, extra).await
}

async fn execute_operation(
    harness: &Harness,
    operation: ProtocolOperation,
    model: &str,
    extra: Value,
) -> TestResponse {
    execute_operation_with_headers(harness, operation, model, extra, HeaderMap::new()).await
}

async fn execute_operation_with_headers(
    harness: &Harness,
    operation: ProtocolOperation,
    model: &str,
    extra: Value,
    headers: HeaderMap,
) -> TestResponse {
    let mut body = extra;
    body["model"] = Value::String(model.to_owned());
    let request_id = RequestId::new();
    let response = harness
        .service
        .execute(
            Arc::clone(&harness.snapshot),
            PublicRequest {
                request_id,
                gateway_api_key_id: GatewayApiKeyId::new(),
                client_ip: "127.0.0.1".parse().expect("client IP"),
                operation,
                headers,
                body: Bytes::from(serde_json::to_vec(&body).expect("request JSON")),
            },
        )
        .await;
    TestResponse::from_response(request_id, response)
}

async fn execute_stream(harness: &Harness, request_id: RequestId, model: &str) -> PublicResponse {
    execute_stream_operation(
        harness,
        request_id,
        ProtocolOperation::Responses,
        model,
        json!({"input":"hello"}),
    )
    .await
}

async fn execute_stream_operation(
    harness: &Harness,
    request_id: RequestId,
    operation: ProtocolOperation,
    model: &str,
    extra: Value,
) -> PublicResponse {
    let mut body = extra;
    body["model"] = Value::String(model.to_owned());
    body["stream"] = Value::Bool(true);
    harness
        .service
        .execute(
            Arc::clone(&harness.snapshot),
            PublicRequest {
                request_id,
                gateway_api_key_id: GatewayApiKeyId::new(),
                client_ip: "127.0.0.1".parse().expect("client IP"),
                operation,
                headers: HeaderMap::new(),
                body: Bytes::from(serde_json::to_vec(&body).expect("stream request JSON")),
            },
        )
        .await
}

fn streaming_body(response: PublicResponse) -> any2api_runtime::api::PublicResponseStream {
    assert_eq!(response.status, StatusCode::OK);
    match response.body {
        PublicResponseBody::Streaming(body) => body,
        PublicResponseBody::Buffered(_) => panic!("test expected streaming response"),
    }
}

struct TestResponse {
    request_id: RequestId,
    status: StatusCode,
    headers: HeaderMap,
    raw_body: Bytes,
}

impl TestResponse {
    fn from_response(request_id: RequestId, response: PublicResponse) -> Self {
        let raw_body = match response.body {
            PublicResponseBody::Buffered(body) => body,
            PublicResponseBody::Streaming(_) => panic!("test expected buffered response"),
        };
        Self {
            request_id,
            status: response.status,
            headers: response.headers,
            raw_body,
        }
    }

    fn status(&self) -> StatusCode {
        self.status
    }

    fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    fn body_bytes(&self) -> &[u8] {
        &self.raw_body
    }

    fn json_body(&self) -> Value {
        serde_json::from_slice(&self.raw_body).expect("JSON response body")
    }
}

async fn wait_for_log(harness: &Harness, request_id: RequestId) -> CompletedRequestLog {
    for _ in 0..200 {
        if let Some(record) = harness
            .telemetry
            .get(request_id)
            .await
            .expect("request log query")
        {
            return record;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("request log was not persisted");
}

#[derive(Clone, Debug)]
struct TransportCall {
    uri: String,
    authorization: Option<String>,
    turn_state: Option<String>,
    body: Bytes,
    read_timeout: Duration,
}

enum ScriptStep {
    Error(TransportError),
    Json {
        status: StatusCode,
        headers: HeaderMap,
        body: Bytes,
    },
    Stream {
        body: Bytes,
    },
    DelayedJson {
        delay: Duration,
        status: StatusCode,
        body: Bytes,
    },
    DelayedStalledStreamWithGap {
        first_delay: Duration,
        first: Bytes,
        second_delay: Duration,
        second: Bytes,
    },
    StalledJson {
        status: StatusCode,
        headers: HeaderMap,
        body: Bytes,
    },
    StalledStream {
        body: Bytes,
    },
    StreamError(TransportError),
}

impl ScriptStep {
    fn json(status: StatusCode, body: &'static str) -> Self {
        Self::Json {
            status,
            headers: HeaderMap::new(),
            body: Bytes::from_static(body.as_bytes()),
        }
    }

    fn json_with_headers(status: StatusCode, headers: HeaderMap, body: &'static str) -> Self {
        Self::Json {
            status,
            headers,
            body: Bytes::from_static(body.as_bytes()),
        }
    }

    fn stream(body: &'static str) -> Self {
        Self::Stream {
            body: Bytes::from_static(body.as_bytes()),
        }
    }

    fn delayed_json(delay: Duration, status: StatusCode, body: &'static str) -> Self {
        Self::DelayedJson {
            delay,
            status,
            body: Bytes::from_static(body.as_bytes()),
        }
    }

    fn stalled_json(status: StatusCode, body: &'static str) -> Self {
        Self::StalledJson {
            status,
            headers: HeaderMap::new(),
            body: Bytes::from_static(body.as_bytes()),
        }
    }

    fn stalled_json_with_headers(
        status: StatusCode,
        headers: HeaderMap,
        body: &'static str,
    ) -> Self {
        Self::StalledJson {
            status,
            headers,
            body: Bytes::from_static(body.as_bytes()),
        }
    }

    fn stalled_stream(body: &'static str) -> Self {
        Self::StalledStream {
            body: Bytes::from_static(body.as_bytes()),
        }
    }

    fn stream_error(error: TransportError) -> Self {
        Self::StreamError(error)
    }
}

struct ScriptedTransport {
    steps: Mutex<VecDeque<ScriptStep>>,
    calls: Mutex<Vec<TransportCall>>,
}

impl ScriptedTransport {
    fn new(steps: impl IntoIterator<Item = ScriptStep>) -> Self {
        Self {
            steps: Mutex::new(steps.into_iter().collect()),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<TransportCall> {
        self.calls.lock().expect("calls lock").clone()
    }
}

#[async_trait]
impl TransportManager for ScriptedTransport {
    async fn execute(
        &self,
        _proxy: TransportProxy<'_>,
        request: TransportRequest,
    ) -> Result<TransportResponse, TransportError> {
        self.calls.lock().expect("calls lock").push(TransportCall {
            uri: request.uri.to_string(),
            authorization: request
                .headers
                .get(header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned),
            turn_state: request
                .headers
                .get("x-codex-turn-state")
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned),
            body: request.body.clone(),
            read_timeout: request.read_timeout,
        });
        let step = self.steps.lock().expect("steps lock").pop_front();
        match step.expect("scripted transport step") {
            ScriptStep::Error(error) => Err(error),
            ScriptStep::Json {
                status,
                headers,
                body,
            } => Ok(TransportResponse {
                status,
                headers,
                body: boxed_body(stream::iter([Ok(body)])),
                read_failure_scope: TransportFailureScope::Endpoint,
            }),
            ScriptStep::Stream { body } => Ok(TransportResponse {
                status: StatusCode::OK,
                headers: HeaderMap::new(),
                body: boxed_body(stream::iter([Ok(body)])),
                read_failure_scope: TransportFailureScope::Endpoint,
            }),
            ScriptStep::DelayedJson {
                delay,
                status,
                body,
            } => Ok(TransportResponse {
                status,
                headers: HeaderMap::new(),
                body: boxed_body(stream::once(async move {
                    tokio::time::sleep(delay).await;
                    Ok(body)
                })),
                read_failure_scope: TransportFailureScope::Endpoint,
            }),
            ScriptStep::DelayedStalledStreamWithGap {
                first_delay,
                first,
                second_delay,
                second,
            } => Ok(TransportResponse {
                status: StatusCode::OK,
                headers: HeaderMap::new(),
                body: boxed_body(
                    stream::once(async move {
                        tokio::time::sleep(first_delay).await;
                        Ok(first)
                    })
                    .chain(stream::once(async move {
                        tokio::time::sleep(second_delay).await;
                        Ok(second)
                    }))
                    .chain(stream::pending()),
                ),
                read_failure_scope: TransportFailureScope::Endpoint,
            }),
            ScriptStep::StalledJson {
                status,
                headers,
                body,
            } => Ok(TransportResponse {
                status,
                headers,
                body: boxed_body(stream::iter([Ok(body)]).chain(stream::pending())),
                read_failure_scope: TransportFailureScope::Endpoint,
            }),
            ScriptStep::StalledStream { body } => Ok(TransportResponse {
                status: StatusCode::OK,
                headers: HeaderMap::new(),
                body: boxed_body(stream::iter([Ok(body)]).chain(stream::pending())),
                read_failure_scope: TransportFailureScope::Endpoint,
            }),
            ScriptStep::StreamError(error) => Ok(TransportResponse {
                status: StatusCode::OK,
                headers: HeaderMap::new(),
                body: boxed_body(stream::iter([Err(error)])),
                read_failure_scope: TransportFailureScope::Endpoint,
            }),
        }
    }
}

fn boxed_body<S>(stream: S) -> BoxByteStream
where
    S: futures_util::Stream<Item = Result<Bytes, TransportError>> + Send + 'static,
{
    Box::pin(stream)
}
