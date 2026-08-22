use any2api_domain::{
    ProtocolDialect, ProtocolOperation, ProviderBaseUrl, RequestSpeedTier, TransportMode,
    UpstreamErrorKind,
};
use http::{
    HeaderMap, StatusCode,
    header::{AUTHORIZATION, CONTENT_TYPE},
};

use super::ClaudeDriver;
use crate::api::{
    OAuthAuthorizationCodeProvider, OAuthRoutingProvider, OAuthTokenProvider, ProviderDriver,
    ProviderRequestContext, ProviderSecret, UpstreamResponseMeta,
};

#[test]
fn gates_speed_tier_to_messages_requests() {
    let driver = ClaudeDriver::new();
    let headers = HeaderMap::new();
    let context = |operation| ProviderRequestContext {
        ingress_dialect: ProtocolDialect::AnthropicMessages,
        upstream_operation: operation,
        upstream_model: "claude",
        client_headers: &headers,
        oauth: false,
        allow_credential_bound: true,
        allow_session_replay: true,
        allow_turn_state: false,
    };

    assert_eq!(
        driver.request_speed_tier(
            context(ProtocolOperation::Messages),
            Some(RequestSpeedTier::Fast),
        ),
        Some(RequestSpeedTier::Fast)
    );
    assert_eq!(
        driver.response_speed_tier(ProtocolOperation::Messages, Some(RequestSpeedTier::Fast),),
        Some(RequestSpeedTier::Fast)
    );
    assert_eq!(
        driver.request_speed_tier(
            context(ProtocolOperation::MessagesCountTokens),
            Some(RequestSpeedTier::Fast),
        ),
        None
    );
    assert_eq!(
        driver.response_speed_tier(
            ProtocolOperation::MessagesCountTokens,
            Some(RequestSpeedTier::Fast),
        ),
        None
    );
}

#[test]
fn builds_messages_paths_and_anthropic_headers() {
    let driver = ClaudeDriver::new();
    let base = ProviderBaseUrl::parse("https://api.example.com/gateway").expect("base URL");
    assert_eq!(
        driver
            .endpoint_plan(&base, ProtocolOperation::MessagesCountTokens)
            .expect("endpoint")
            .url
            .as_str(),
        "https://api.example.com/gateway/v1/messages/count_tokens"
    );
    assert_eq!(
        driver
            .credential_test_plan(&base)
            .expect("credential test endpoint")
            .url
            .as_str(),
        "https://api.example.com/gateway/v1/models"
    );
    assert_eq!(
        driver
            .credential_test_plan(&base)
            .expect("credential test endpoint")
            .headers["anthropic-version"],
        "2023-06-01"
    );
    let headers = driver
        .credential_headers(&base, &ProviderSecret::new("sk-claude"))
        .expect("headers");
    assert_eq!(headers.headers[AUTHORIZATION], "Bearer sk-claude");
    assert!(!headers.headers.contains_key("x-api-key"));
    assert!(!headers.headers.contains_key("anthropic-version"));
    let official = ProviderBaseUrl::parse("https://api.anthropic.com").expect("official base URL");
    let official_headers = driver
        .credential_headers(&official, &ProviderSecret::new("sk-ant-official"))
        .expect("official headers");
    assert_eq!(official_headers.headers["x-api-key"], "sk-ant-official");
    assert!(!official_headers.headers.contains_key(AUTHORIZATION));
    let identity = driver
        .prepare_request_headers(ProviderRequestContext {
            ingress_dialect: ProtocolDialect::AnthropicMessages,
            upstream_operation: ProtocolOperation::Messages,
            upstream_model: "claude",
            client_headers: &HeaderMap::new(),
            oauth: false,
            allow_credential_bound: true,
            allow_session_replay: true,
            allow_turn_state: false,
        })
        .expect("identity headers");
    assert_eq!(identity["anthropic-version"], "2023-06-01");
    assert!(
        driver
            .descriptor()
            .supports_transport_mode(TransportMode::Sse)
    );
    assert!(
        !driver
            .descriptor()
            .supports_protocol(ProtocolDialect::OpenAiImages)
    );
    assert!(
        driver
            .endpoint_plan(&base, ProtocolOperation::ImagesGenerations)
            .is_err()
    );
    assert!(
        driver
            .endpoint_plan(&base, ProtocolOperation::ImagesEdits)
            .is_err()
    );
    assert!(
        !driver
            .descriptor()
            .supports_oauth_operation(ProtocolOperation::ImagesGenerations)
    );
    assert!(
        !driver
            .descriptor()
            .supports_oauth_operation(ProtocolOperation::ImagesEdits)
    );
}

#[test]
fn count_tokens_not_found_is_operation_unavailable() {
    let driver = ClaudeDriver::new();
    let response = UpstreamResponseMeta {
        status: StatusCode::NOT_FOUND,
        headers: HeaderMap::new(),
    };

    assert_eq!(
        driver
            .classify_error(ProtocolOperation::MessagesCountTokens, &response, b"{}")
            .classification()
            .kind(),
        UpstreamErrorKind::OperationUnavailable
    );
    assert_eq!(
        driver
            .classify_error(ProtocolOperation::Messages, &response, b"{}")
            .classification()
            .kind(),
        UpstreamErrorKind::ModelUnavailable
    );
}

#[test]
fn builds_pkce_json_token_request() {
    let driver = ClaudeDriver::new();
    let authorization = driver
        .oauth_authorization_url("state-value", "challenge-value")
        .expect("authorization URL");
    let query: std::collections::HashMap<_, _> = authorization.query_pairs().into_owned().collect();
    assert_eq!(query.get("state").map(String::as_str), Some("state-value"));
    assert_eq!(
        query.get("code_challenge").map(String::as_str),
        Some("challenge-value")
    );
    let plan = driver
        .oauth_authorization_code_token_request(
            "authorization-code",
            "state-value",
            "verifier-value",
        )
        .expect("token request");
    assert_eq!(plan.headers[CONTENT_TYPE], "application/json");
    let body: serde_json::Value = serde_json::from_slice(&plan.body).expect("token JSON");
    assert_eq!(body["code"], "authorization-code");
    assert_eq!(body["state"], "state-value");
    assert_eq!(body["code_verifier"], "verifier-value");
    assert!(!format!("{plan:?}").contains("verifier-value"));

    let refresh = driver
        .oauth_refresh_token_request("refresh-secret")
        .expect("refresh request");
    let body: serde_json::Value = serde_json::from_slice(&refresh.body).expect("refresh JSON");
    assert_eq!(body["grant_type"], "refresh_token");
    assert_eq!(body["refresh_token"], "refresh-secret");
    assert!(!format!("{refresh:?}").contains("refresh-secret"));
}

#[test]
fn parses_claude_account_email() {
    let driver = ClaudeDriver::new();
    let token = driver
        .parse_oauth_token_response(
            br#"{"access_token":"access-secret","refresh_token":"refresh-secret","expires_in":3600,"account":{"email_address":"claude@example.com"}}"#,
        )
        .expect("token response");
    assert_eq!(token.email(), Some("claude@example.com"));
    assert!(!format!("{token:?}").contains("claude@example.com"));
    let profile = driver
        .oauth_routing_profile(&token)
        .expect("OAuth routing profile");
    assert_eq!(profile.base_url().as_str(), "https://api.anthropic.com");
    assert_eq!(
        profile.protocol_dialect(),
        ProtocolDialect::AnthropicMessages
    );
    assert_eq!(
        driver
            .endpoint_plan(profile.base_url(), ProtocolOperation::Messages)
            .expect("OAuth endpoint")
            .url
            .as_str(),
        "https://api.anthropic.com/v1/messages"
    );
}

#[test]
fn builds_claude_oauth_headers_and_preserves_client_betas() {
    let driver = ClaudeDriver::new();
    let token = driver
        .parse_oauth_token_response(br#"{"access_token":"oauth-secret","expires_in":3600}"#)
        .expect("OAuth token response");
    let mut forwarded = HeaderMap::new();
    forwarded.append(
        "anthropic-beta",
        "custom-beta".parse().expect("beta header"),
    );
    forwarded.append(
        "anthropic-beta",
        "second-beta".parse().expect("second beta header"),
    );
    let headers = driver
        .oauth_credential_headers(&token, &forwarded)
        .expect("OAuth headers");

    assert_eq!(headers.headers["authorization"], "Bearer oauth-secret");
    assert!(!headers.headers.contains_key("anthropic-version"));
    assert_eq!(
        headers
            .headers
            .get_all("anthropic-beta")
            .iter()
            .map(|value| value.to_str().expect("beta text"))
            .collect::<Vec<_>>(),
        ["custom-beta", "second-beta", "oauth-2025-04-20"]
    );
    let mut already_required = HeaderMap::new();
    already_required.append("anthropic-beta", "first-beta".parse().expect("beta"));
    already_required.append(
        "anthropic-beta",
        "second-beta,oauth-2025-04-20"
            .parse()
            .expect("required beta"),
    );
    let already_required = driver
        .oauth_credential_headers(&token, &already_required)
        .expect("OAuth headers with required beta");
    assert_eq!(
        already_required
            .headers
            .get_all("anthropic-beta")
            .iter()
            .map(|value| value.to_str().expect("beta text"))
            .collect::<Vec<_>>(),
        ["first-beta", "second-beta,oauth-2025-04-20"]
    );
    let identity = driver
        .prepare_request_headers(ProviderRequestContext {
            ingress_dialect: ProtocolDialect::AnthropicMessages,
            upstream_operation: ProtocolOperation::Messages,
            upstream_model: "claude",
            client_headers: &forwarded,
            oauth: true,
            allow_credential_bound: true,
            allow_session_replay: true,
            allow_turn_state: false,
        })
        .expect("identity headers");
    assert_eq!(identity["anthropic-version"], "2023-06-01");
    assert!(!format!("{headers:?}").contains("oauth-secret"));
}
