use any2api_domain::{
    ProtocolDialect, ProtocolOperation, ProviderBaseUrl, ProviderKind, TransportMode,
    UpstreamErrorKind,
};
use any2api_protocol::api::{OpenAiChatCompletionsProfile, ProtocolTargetProfile};
use bytes::Bytes;
use http::{HeaderMap, HeaderValue, StatusCode, header::AUTHORIZATION};

use super::OpenAiDriver;
use crate::api::{ProviderDriver, ProviderRequestContext, ProviderSecret, UpstreamResponseMeta};

#[test]
fn declares_the_standard_openai_contract_and_target_profile() {
    let driver = OpenAiDriver::new();

    assert_eq!(driver.kind(), ProviderKind::OpenAi);
    assert_eq!(
        driver.descriptor().protocols().collect::<Vec<_>>(),
        vec![
            ProtocolDialect::OpenAiResponses,
            ProtocolDialect::OpenAiChatCompletions,
            ProtocolDialect::OpenAiImages,
        ]
    );
    assert_eq!(
        driver.descriptor().transport_modes(),
        &[TransportMode::Json, TransportMode::Sse]
    );
    assert_eq!(
        driver.protocol_target_profile(ProtocolDialect::OpenAiChatCompletions, "gpt-5.4"),
        Some(ProtocolTargetProfile::OpenAiChatCompletions(
            OpenAiChatCompletionsProfile::CURRENT_OPENAI,
        ))
    );
    assert!(driver.descriptor().oauth().is_none());
    assert!(driver.oauth_token().is_none());
}

#[test]
fn builds_only_standard_openai_paths_and_bearer_authentication() {
    let driver = OpenAiDriver::new();
    let base = ProviderBaseUrl::parse("https://api.openai.com/v1").expect("base URL");
    for (operation, expected) in [
        (ProtocolOperation::Responses, "responses"),
        (ProtocolOperation::ResponsesCompact, "responses/compact"),
        (ProtocolOperation::ChatCompletions, "chat/completions"),
        (ProtocolOperation::ImagesGenerations, "images/generations"),
        (ProtocolOperation::ImagesEdits, "images/edits"),
    ] {
        assert!(driver.descriptor().supports_api_key_operation(operation));
        assert_eq!(
            driver
                .endpoint_plan(&base, operation)
                .expect("standard endpoint")
                .url
                .as_str(),
            format!("https://api.openai.com/v1/{expected}")
        );
    }
    for unsupported in [ProtocolOperation::AlphaSearch, ProtocolOperation::Messages] {
        assert!(!driver.descriptor().supports_api_key_operation(unsupported));
        assert!(driver.endpoint_plan(&base, unsupported).is_err());
    }
    assert_eq!(
        driver
            .credential_test_plan(&base)
            .expect("models endpoint")
            .url
            .as_str(),
        "https://api.openai.com/v1/models"
    );
    let headers = driver
        .credential_headers(&base, &ProviderSecret::new("sk-openai"))
        .expect("Bearer headers");
    assert_eq!(headers.headers[AUTHORIZATION], "Bearer sk-openai");
    assert!(!format!("{headers:?}").contains("sk-openai"));
}

#[test]
fn leaves_request_identity_untouched_and_projects_only_safe_response_headers() {
    let driver = OpenAiDriver::new();
    let mut client_headers = HeaderMap::new();
    client_headers.insert("user-agent", HeaderValue::from_static("codex-cli/borrowed"));
    client_headers.insert("x-oai-attestation", HeaderValue::from_static("borrowed"));
    let context = ProviderRequestContext {
        ingress_dialect: ProtocolDialect::OpenAiResponses,
        upstream_operation: ProtocolOperation::Responses,
        upstream_model: "gpt-5.4",
        client_headers: &client_headers,
        oauth: false,
        allow_credential_bound: true,
        allow_turn_state: false,
        allow_session_replay: true,
    };
    assert!(
        driver
            .prepare_request_headers(context)
            .expect("request headers")
            .is_empty()
    );
    let body = Bytes::from_static(br#"{"model":"gpt-5.4"}"#);
    assert_eq!(
        driver
            .prepare_request_body(context, body.clone())
            .expect("request body"),
        body
    );

    let mut upstream = HeaderMap::new();
    for (name, value) in [
        ("content-type", "application/json"),
        ("x-request-id", "openai-request"),
        ("openai-processing-ms", "41"),
        ("x-ratelimit-remaining-requests", "9"),
        ("server", "private"),
        ("set-cookie", "secret=value"),
    ] {
        upstream.insert(name, HeaderValue::from_static(value));
    }
    let response = driver.response_headers(ProtocolOperation::Responses, &upstream);
    assert_eq!(response["x-request-id"], "openai-request");
    assert_eq!(response["openai-processing-ms"], "41");
    assert_eq!(response["x-ratelimit-remaining-requests"], "9");
    assert!(!response.contains_key("server"));
    assert!(!response.contains_key("set-cookie"));
}

#[test]
fn uses_structured_openai_error_classification() {
    let classified = OpenAiDriver::new().classify_error(
        ProtocolOperation::Responses,
        &UpstreamResponseMeta {
            status: StatusCode::TOO_MANY_REQUESTS,
            headers: HeaderMap::new(),
        },
        br#"{"error":{"type":"insufficient_quota","message":"quota detail"}}"#,
    );

    assert_eq!(
        classified.classification().kind(),
        UpstreamErrorKind::QuotaExhausted
    );
    assert_eq!(classified.official_message(), Some("quota detail"));
}
