use any2api_domain::{
    OpenAiChatCachedTokensField, OpenAiChatReasoningRequest, OpenAiChatTokenLimitField,
    ProtocolDialect, ProtocolOperation, ProtocolTargetProfile, ProviderBaseUrl, ProviderKind,
    RetrySafety, TransportMode, UpstreamErrorKind, UpstreamFailureAttribution,
};
use http::{HeaderMap, HeaderValue, StatusCode, header::AUTHORIZATION};

use super::KimiDriver;
use crate::api::{ProviderDriver, ProviderRequestContext, ProviderSecret, UpstreamResponseMeta};

#[test]
fn declares_model_aware_kimi_chat_target_profiles() {
    let driver = KimiDriver::new();

    let Some(ProtocolTargetProfile::OpenAiChatCompletions(k3)) =
        driver.protocol_target_profile(ProtocolDialect::OpenAiChatCompletions, "kimi-k3")
    else {
        panic!("Kimi Chat target profile");
    };
    assert_eq!(
        k3.token_limit_field,
        OpenAiChatTokenLimitField::MaxCompletionTokens
    );
    assert_eq!(
        k3.reasoning_request,
        OpenAiChatReasoningRequest::ReasoningEffort
    );
    assert_eq!(
        k3.cached_tokens_field,
        OpenAiChatCachedTokensField::TopLevel
    );
    assert!(!k3.supports_image_detail);

    let Some(ProtocolTargetProfile::OpenAiChatCompletions(k2)) =
        driver.protocol_target_profile(ProtocolDialect::OpenAiChatCompletions, "kimi-k2.6")
    else {
        panic!("Kimi Chat target profile");
    };
    assert_eq!(
        k2.reasoning_request,
        OpenAiChatReasoningRequest::Unsupported
    );
    assert_eq!(
        driver.protocol_target_profile(ProtocolDialect::OpenAiResponses, "kimi-k3"),
        None
    );
}

#[test]
fn exposes_only_chat_completions_and_builds_moonshot_paths() {
    let driver = KimiDriver::new();
    let base = ProviderBaseUrl::parse("https://api.moonshot.cn/v1").expect("base URL");

    assert_eq!(driver.kind(), ProviderKind::Kimi);
    assert_eq!(
        driver.capabilities().protocols,
        [ProtocolDialect::OpenAiChatCompletions]
            .into_iter()
            .collect()
    );
    assert_eq!(
        driver.capabilities().transport_modes,
        [TransportMode::Json, TransportMode::Sse]
            .into_iter()
            .collect()
    );
    assert_eq!(
        driver
            .endpoint_plan(&base, ProtocolOperation::ChatCompletions)
            .expect("chat endpoint")
            .url
            .as_str(),
        "https://api.moonshot.cn/v1/chat/completions"
    );
    assert_eq!(
        driver
            .credential_test_plan(&base)
            .expect("models endpoint")
            .url
            .as_str(),
        "https://api.moonshot.cn/v1/models"
    );
    for unsupported in [
        ProtocolOperation::Responses,
        ProtocolOperation::ResponsesCompact,
        ProtocolOperation::ImagesGenerations,
        ProtocolOperation::Messages,
    ] {
        assert!(driver.endpoint_plan(&base, unsupported).is_err());
    }
    assert!(driver.oauth_login_flow().is_none());

    let headers = driver
        .credential_headers(&base, &ProviderSecret::new("sk-kimi"))
        .expect("Bearer headers");
    assert_eq!(headers.headers[AUTHORIZATION], "Bearer sk-kimi");
    assert!(!format!("{headers:?}").contains("sk-kimi"));
}

#[test]
fn parses_model_catalog_without_model_name_special_cases() {
    let models = KimiDriver::new()
        .parse_model_catalog(br#"{"data":[{"id":"kimi-k3"},{"id":"kimi-k2.5"}]}"#)
        .expect("model catalog");
    assert_eq!(models, ["kimi-k2.5", "kimi-k3"]);
}

#[test]
fn emits_no_borrowed_persona_and_projects_only_kimi_response_headers() {
    let driver = KimiDriver::new();
    let mut client_headers = HeaderMap::new();
    for (name, value) in [
        ("user-agent", "codex-cli/borrowed"),
        ("traceparent", "00-client-trace"),
        ("x-grok-client-version", "borrowed"),
        ("x-stainless-runtime", "borrowed"),
    ] {
        client_headers.insert(name, HeaderValue::from_static(value));
    }
    let request = driver
        .prepare_request_headers(ProviderRequestContext {
            ingress_dialect: ProtocolDialect::OpenAiResponses,
            upstream_operation: ProtocolOperation::ChatCompletions,
            upstream_model: "kimi-k3",
            client_headers: &client_headers,
            oauth: false,
            allow_credential_bound: true,
            allow_turn_state: false,
            allow_session_replay: true,
        })
        .expect("request headers");
    assert!(request.is_empty());

    let mut upstream = HeaderMap::new();
    for (name, value) in [
        ("content-type", "application/json"),
        ("x-request-id", "kimi-request"),
        ("retry-after", "3"),
        ("x-ratelimit-remaining-requests", "9"),
        ("server", "nginx"),
        ("set-cookie", "secret=value"),
    ] {
        upstream.insert(name, HeaderValue::from_static(value));
    }
    let response = driver.response_headers(ProtocolOperation::ChatCompletions, &upstream);
    assert_eq!(response["x-request-id"], "kimi-request");
    assert_eq!(response["x-ratelimit-remaining-requests"], "9");
    assert!(!response.contains_key("server"));
    assert!(!response.contains_key("set-cookie"));
}

#[test]
fn classifies_declared_moonshot_errors_without_message_matching() {
    let driver = KimiDriver::new();
    for (status, kind, expected) in [
        (
            StatusCode::UNAUTHORIZED,
            "incorrect_api_key_error",
            UpstreamErrorKind::Authentication,
        ),
        (
            StatusCode::FORBIDDEN,
            "permission_denied_error",
            UpstreamErrorKind::PermissionDenied,
        ),
        (
            StatusCode::NOT_FOUND,
            "resource_not_found_error",
            UpstreamErrorKind::ModelUnavailable,
        ),
        (
            StatusCode::TOO_MANY_REQUESTS,
            "exceeded_current_quota_error",
            UpstreamErrorKind::QuotaExhausted,
        ),
        (
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limit_reached_error",
            UpstreamErrorKind::RateLimited,
        ),
        (
            StatusCode::BAD_REQUEST,
            "content_filter",
            UpstreamErrorKind::InvalidRequest,
        ),
    ] {
        let body = serde_json::json!({
            "error": {"type": kind, "message": "Moonshot detail"}
        });
        let classified = driver.classify_error(
            ProtocolOperation::ChatCompletions,
            &UpstreamResponseMeta {
                status,
                headers: HeaderMap::new(),
            },
            body.to_string().as_bytes(),
        );
        assert_eq!(classified.classification().kind(), expected, "{kind}");
        assert_eq!(classified.official_message(), Some("Moonshot detail"));
    }

    let message_only = driver.classify_error(
        ProtocolOperation::ChatCompletions,
        &UpstreamResponseMeta {
            status: StatusCode::BAD_REQUEST,
            headers: HeaderMap::new(),
        },
        br#"{"error":{"message":"quota exhausted"}}"#,
    );
    assert_eq!(
        message_only.classification().kind(),
        UpstreamErrorKind::InvalidRequest
    );
}

#[test]
fn engine_overload_is_service_transient_not_credential_rate_limit() {
    let driver = KimiDriver::new();
    let mut headers = HeaderMap::new();
    headers.insert("retry-after", HeaderValue::from_static("7"));
    let classified = driver.classify_error(
        ProtocolOperation::ChatCompletions,
        &UpstreamResponseMeta {
            status: StatusCode::TOO_MANY_REQUESTS,
            headers,
        },
        br#"{"error":{"type":"engine_overloaded_error"}}"#,
    );

    assert_eq!(
        classified.classification().kind(),
        UpstreamErrorKind::Transient
    );
    assert_eq!(
        classified.classification().retry_safety(),
        RetrySafety::RejectedBeforeExecution
    );
    assert_eq!(
        classified.classification().attribution(),
        UpstreamFailureAttribution::Unattributed
    );
    assert!(classified.classification().retry_after().is_some());
}
