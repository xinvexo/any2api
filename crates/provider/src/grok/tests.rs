//! Grok API Key and OAuth driver contracts.

use any2api_domain::{
    ProtocolDialect, ProtocolOperation, ProviderBaseUrl, ProviderKind, TransportMode,
};
use base64::Engine as _;
use http::{StatusCode, header::AUTHORIZATION, header::CONTENT_TYPE};

use super::{GrokDriver, oauth_bot_flag};
use crate::api::{
    OAuthDeviceTokenPoll, OAuthGrant, OAuthLoginFlow, OAuthTokenMaterial, ProviderDriver,
    ProviderError, ProviderRequestContext, ProviderSecret,
};

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
        .credential_headers(&base, &ProviderSecret::new("xai-test-key"))
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
        !driver
            .capabilities()
            .protocols
            .contains(&ProtocolDialect::OpenAiImages)
    );
    assert!(
        driver
            .capabilities()
            .transport_modes
            .contains(&TransportMode::Sse)
    );
    assert_eq!(driver.oauth_login_flow(), Some(OAuthLoginFlow::DeviceCode));
    assert_eq!(driver.oauth_redirect_uri(), None);
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
    assert!(!driver.oauth_supports_operation(ProtocolOperation::ImagesGenerations));
    assert!(!driver.oauth_supports_operation(ProtocolOperation::ImagesEdits));
}

#[test]
fn builds_grok_device_authorization_and_refresh_requests() {
    let driver = GrokDriver::new();
    let authorization = driver
        .oauth_device_authorization_request()
        .expect("device authorization request");
    assert_eq!(
        authorization.url.as_str(),
        "https://auth.x.ai/oauth2/device/code"
    );
    assert_eq!(
        authorization.headers[CONTENT_TYPE],
        "application/x-www-form-urlencoded"
    );
    let form: std::collections::HashMap<_, _> = url::form_urlencoded::parse(&authorization.body)
        .into_owned()
        .collect();
    assert_eq!(
        form.get("client_id").map(String::as_str),
        Some("b1a00492-073a-47ea-816f-4c329264a828")
    );
    assert_eq!(
        form.get("scope").map(String::as_str),
        Some("openid profile email offline_access grok-cli:access api:access")
    );

    let device = driver
        .parse_oauth_device_authorization(
            br#"{"device_code":"device-secret","user_code":"ABCD-1234","verification_uri":"https://accounts.x.ai/oauth2/device","verification_uri_complete":"https://accounts.x.ai/oauth2/device?user_code=ABCD-1234","expires_in":1800,"interval":2}"#,
        )
        .expect("device authorization response");
    assert_eq!(device.user_code(), "ABCD-1234");
    assert_eq!(device.poll_interval_seconds(), 5);
    assert_eq!(device.expires_in_seconds(), 1800);
    assert!(!format!("{device:?}").contains("device-secret"));

    let exchange = driver
        .oauth_device_token_request(device.device_code())
        .expect("device token request");
    assert_eq!(
        exchange.headers[CONTENT_TYPE],
        "application/x-www-form-urlencoded"
    );
    let form: std::collections::HashMap<_, _> = url::form_urlencoded::parse(&exchange.body)
        .into_owned()
        .collect();
    assert_eq!(
        form.get("grant_type").map(String::as_str),
        Some("urn:ietf:params:oauth:grant-type:device_code")
    );
    assert_eq!(
        form.get("device_code").map(String::as_str),
        Some("device-secret")
    );
    assert!(!format!("{exchange:?}").contains("device-secret"));
    assert!(
        driver
            .oauth_token_request(OAuthGrant::AuthorizationCode, "code", None, None)
            .is_err()
    );

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
fn classifies_grok_device_poll_responses() {
    let driver = GrokDriver::new();
    assert!(matches!(
        driver
            .parse_oauth_device_token(
                StatusCode::BAD_REQUEST,
                br#"{"error":"authorization_pending"}"#,
            )
            .expect("pending response"),
        OAuthDeviceTokenPoll::Pending
    ));
    assert!(matches!(
        driver
            .parse_oauth_device_token(StatusCode::BAD_REQUEST, br#"{"error":"slow_down"}"#)
            .expect("slow-down response"),
        OAuthDeviceTokenPoll::SlowDown
    ));
    assert!(matches!(
        driver
            .parse_oauth_device_token(StatusCode::BAD_REQUEST, br#"{"error":"access_denied"}"#)
            .expect("denied response"),
        OAuthDeviceTokenPoll::Denied
    ));
    assert!(matches!(
        driver
            .parse_oauth_device_token(StatusCode::BAD_REQUEST, br#"{"error":"expired_token"}"#)
            .expect("expired response"),
        OAuthDeviceTokenPoll::Expired
    ));

    let authorized = driver
        .parse_oauth_device_token(
            StatusCode::OK,
            br#"{"access_token":"access-secret","refresh_token":"refresh-secret","expires_in":3600}"#,
        )
        .expect("authorized response");
    let OAuthDeviceTokenPoll::Authorized(token) = authorized else {
        panic!("expected an authorized token")
    };
    assert_eq!(token.provider(), ProviderKind::Grok);
    assert!(!format!("{token:?}").contains("access-secret"));
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
        .parse_oauth_token_response(body.as_bytes())
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
    assert!(!driver.oauth_supports_operation(ProtocolOperation::ImagesGenerations));
    assert!(!driver.oauth_supports_operation(ProtocolOperation::ImagesEdits));

    let headers = driver
        .oauth_credential_headers(&token, &http::HeaderMap::new())
        .expect("OAuth headers");
    assert_eq!(headers.headers[AUTHORIZATION], "Bearer access-secret");
    assert_eq!(headers.headers["x-userid"], "subject-1");
    assert!(!headers.headers.contains_key("x-xai-token-auth"));
    let identity = driver
        .prepare_request_headers(ProviderRequestContext {
            ingress_dialect: ProtocolDialect::OpenAiResponses,
            upstream_operation: ProtocolOperation::Responses,
            upstream_model: "grok-4.5",
            client_headers: &http::HeaderMap::new(),
            oauth: true,
            allow_credential_bound: true,
            allow_turn_state: false,
        })
        .expect("identity headers");
    assert_eq!(identity["x-xai-token-auth"], "xai-grok-cli");
    assert_eq!(identity["x-grok-client-version"], "0.2.112");
    assert_eq!(identity["user-agent"], super::identity::user_agent_text());
    assert!(!format!("{headers:?}").contains("access-secret"));
}

#[test]
fn grok_oauth_model_header_preserves_utf8_without_restricting_api_keys() {
    let driver = GrokDriver::new();
    let client_headers = http::HeaderMap::new();

    let oauth_headers = driver
        .prepare_request_headers(ProviderRequestContext {
            ingress_dialect: ProtocolDialect::OpenAiResponses,
            upstream_operation: ProtocolOperation::Responses,
            upstream_model: "本地/Grok",
            client_headers: &client_headers,
            oauth: true,
            allow_credential_bound: true,
            allow_turn_state: false,
        })
        .expect("UTF-8 Grok OAuth model header");
    assert_eq!(
        oauth_headers["x-grok-model-override"].as_bytes(),
        "本地/Grok".as_bytes()
    );

    let error = driver
        .prepare_request_headers(ProviderRequestContext {
            ingress_dialect: ProtocolDialect::OpenAiResponses,
            upstream_operation: ProtocolOperation::Responses,
            upstream_model: "invalid\nmodel",
            client_headers: &client_headers,
            oauth: true,
            allow_credential_bound: true,
            allow_turn_state: false,
        })
        .expect_err("control bytes are not legal in an HTTP header");
    assert_eq!(
        error,
        ProviderError::UnsupportedOAuthModel {
            provider: ProviderKind::Grok,
            model: "invalid\nmodel".to_owned(),
        }
    );

    let api_key_headers = driver
        .prepare_request_headers(ProviderRequestContext {
            ingress_dialect: ProtocolDialect::OpenAiResponses,
            upstream_operation: ProtocolOperation::Responses,
            upstream_model: "本地/Grok",
            client_headers: &client_headers,
            oauth: false,
            allow_credential_bound: true,
            allow_turn_state: false,
        })
        .expect("API Key model stays in the JSON request body");
    assert!(!api_key_headers.contains_key("x-grok-model-override"));
}

#[test]
fn grok_jwt_identity_falls_back_per_claim_field() {
    let driver = GrokDriver::new();
    for (id_payload, access_payload, expected_subject, expected_email) in [
        (
            br#"{"email":"id@example.com"}"#.as_slice(),
            br#"{"sub":"access-subject"}"#.as_slice(),
            "access-subject",
            "id@example.com",
        ),
        (
            br#"{"sub":"id-subject"}"#.as_slice(),
            br#"{"email":"access@example.com"}"#.as_slice(),
            "id-subject",
            "access@example.com",
        ),
    ] {
        let id_claims = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(id_payload);
        let access_claims = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(access_payload);
        let id_token = format!("header.{id_claims}.id-secret");
        let access_token = format!("header.{access_claims}.access-secret");
        let body = serde_json::to_vec(&serde_json::json!({
            "access_token": access_token,
            "id_token": id_token,
            "sub": "response-subject",
            "email": "response@example.com",
            "expires_in": 3600
        }))
        .expect("token response");

        let token = driver
            .parse_oauth_token_response(&body)
            .expect("Grok token");

        assert_eq!(token.account_id(), Some(expected_subject));
        assert_eq!(token.email(), Some(expected_email));
        let debug = format!("{token:?}");
        assert!(!debug.contains("access-secret"));
        assert!(!debug.contains("id-secret"));
    }
}

#[test]
fn derives_only_the_explicit_numeric_build_bot_flag() {
    assert_eq!(oauth_bot_flag(&token_with_bot_claim(1)), Some(true));
    assert_eq!(oauth_bot_flag(&token_with_bot_claim(0)), Some(false));
    assert_eq!(oauth_bot_flag(&token_with_bot_claim(2)), None);

    let claims =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(br#"{"bot_flag_source":"1"}"#);
    let token = OAuthTokenMaterial::new(
        ProviderKind::Grok,
        format!("header.{claims}.signature"),
        None,
        None,
        None,
        Some("subject-1".into()),
        None,
    )
    .expect("token");
    assert_eq!(oauth_bot_flag(&token), None);
}

fn token_with_bot_claim(value: i64) -> OAuthTokenMaterial {
    let claims = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(format!(r#"{{"bot_flag_source":{value}}}"#));
    OAuthTokenMaterial::new(
        ProviderKind::Grok,
        format!("header.{claims}.signature"),
        None,
        None,
        None,
        Some("subject-1".into()),
        None,
    )
    .expect("token")
}
