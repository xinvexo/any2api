//! Grok API Key and OAuth driver contracts.

use any2api_domain::{
    ProtocolDialect, ProtocolOperation, ProviderBaseUrl, ProviderKind, TransportMode,
};
use base64::Engine as _;
use http::{header::AUTHORIZATION, header::CONTENT_TYPE};

use super::GrokDriver;
use crate::{OAuthGrant, ProviderSecret, api::ProviderDriver};

#[test]
fn builds_xai_paths_and_bearer_authentication() {
    let driver = GrokDriver::new();
    let base = ProviderBaseUrl::parse("https://api.x.ai/v1").expect("base URL");

    assert_eq!(driver.kind(), ProviderKind::Grok);
    assert_eq!(
        driver
            .endpoint_plan(&base, ProtocolOperation::ResponsesCompact)
            .expect("compact endpoint")
            .url
            .as_str(),
        "https://api.x.ai/v1/responses/compact"
    );
    assert_eq!(
        driver
            .endpoint_plan(&base, ProtocolOperation::ChatCompletions)
            .expect("chat endpoint")
            .url
            .as_str(),
        "https://api.x.ai/v1/chat/completions"
    );
    assert_eq!(
        driver
            .credential_test_plan(&base)
            .expect("models endpoint")
            .url
            .as_str(),
        "https://api.x.ai/v1/models"
    );
    let headers = driver
        .credential_headers(&ProviderSecret::new(1, "xai-test-key"))
        .expect("headers");
    assert_eq!(headers.headers[AUTHORIZATION], "Bearer xai-test-key");
    assert!(!format!("{headers:?}").contains("xai-test-key"));
    assert!(
        driver
            .capabilities()
            .protocols
            .contains(&ProtocolDialect::OpenAiResponses)
    );
    assert!(
        driver
            .capabilities()
            .transport_modes
            .contains(&TransportMode::Sse)
    );
    assert_eq!(
        driver.oauth_redirect_uri(),
        Some("http://127.0.0.1:56121/callback")
    );
}

#[test]
fn rejects_anthropic_operations() {
    let driver = GrokDriver::new();
    let base = ProviderBaseUrl::parse("https://api.x.ai/v1").expect("base URL");

    assert!(
        driver
            .endpoint_plan(&base, ProtocolOperation::Messages)
            .is_err()
    );
}

#[test]
fn builds_grok_pkce_and_refresh_requests() {
    let driver = GrokDriver::new();
    let authorization = driver
        .oauth_authorization_url("state-value", "challenge-value")
        .expect("authorization URL");
    let query: std::collections::HashMap<_, _> = authorization.query_pairs().into_owned().collect();
    assert_eq!(query.get("state").map(String::as_str), Some("state-value"));
    assert_eq!(query.get("nonce").map(String::as_str), Some("state-value"));
    assert_eq!(
        query.get("scope").map(String::as_str),
        Some("openid profile email offline_access grok-cli:access api:access")
    );

    let exchange = driver
        .oauth_token_request(
            OAuthGrant::AuthorizationCode,
            "authorization-code",
            None,
            Some("verifier-value"),
        )
        .expect("exchange request");
    assert_eq!(
        exchange.headers[CONTENT_TYPE],
        "application/x-www-form-urlencoded"
    );
    let form: std::collections::HashMap<_, _> = url::form_urlencoded::parse(&exchange.body)
        .into_owned()
        .collect();
    assert_eq!(
        form.get("code").map(String::as_str),
        Some("authorization-code")
    );
    assert_eq!(
        form.get("code_verifier").map(String::as_str),
        Some("verifier-value")
    );
    assert!(!format!("{exchange:?}").contains("verifier-value"));

    let refresh = driver
        .oauth_token_request(OAuthGrant::RefreshToken, "refresh-secret", None, None)
        .expect("refresh request");
    let form: std::collections::HashMap<_, _> = url::form_urlencoded::parse(&refresh.body)
        .into_owned()
        .collect();
    assert_eq!(
        form.get("grant_type").map(String::as_str),
        Some("refresh_token")
    );
    assert_eq!(
        form.get("refresh_token").map(String::as_str),
        Some("refresh-secret")
    );
    assert!(!format!("{refresh:?}").contains("refresh-secret"));
}

#[test]
fn parses_grok_oauth_and_builds_subscription_routing() {
    let claims = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(br#"{"email":"grok@example.com","sub":"subject-1"}"#);
    let body = format!(
        r#"{{"access_token":"access-secret","refresh_token":"refresh-secret","id_token":"header.{claims}.signature","expires_in":3600}}"#
    );
    let driver = GrokDriver::new();
    let token = driver
        .parse_oauth_token(body.as_bytes())
        .expect("Grok token response");
    assert_eq!(token.provider(), ProviderKind::Grok);
    assert_eq!(token.email(), Some("grok@example.com"));
    assert_eq!(token.account_id(), Some("subject-1"));
    assert!(!format!("{token:?}").contains("access-secret"));

    let profile = driver
        .oauth_routing_profile(&token)
        .expect("routing profile");
    assert_eq!(
        profile.base_url().as_str(),
        "https://cli-chat-proxy.grok.com/v1"
    );
    assert_eq!(profile.protocol_dialect(), ProtocolDialect::OpenAiResponses);
    assert_eq!(profile.models().len(), 7);
    assert!(driver.oauth_supports_operation(ProtocolOperation::Responses));
    assert!(!driver.oauth_supports_operation(ProtocolOperation::ResponsesCompact));

    let headers = driver
        .oauth_credential_headers(&token, &http::HeaderMap::new())
        .expect("OAuth headers");
    assert_eq!(headers.headers[AUTHORIZATION], "Bearer access-secret");
    assert_eq!(headers.headers["x-xai-token-auth"], "xai-grok-cli");
    assert_eq!(headers.headers["x-grok-client-version"], "0.2.93");
    assert_eq!(headers.headers["user-agent"], "xai-grok-workspace/0.2.93");
    assert!(!format!("{headers:?}").contains("access-secret"));
}
