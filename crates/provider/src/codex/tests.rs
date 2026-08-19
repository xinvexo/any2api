use any2api_domain::{
    ProtocolDialect, ProtocolOperation, ProviderBaseUrl, ProviderKind, QuotaCostUnit,
    RequestSpeedTier, TransportMode,
};
use base64::Engine as _;
use http::{HeaderMap, StatusCode, header::AUTHORIZATION, header::CONTENT_TYPE};

use super::CodexDriver;
use crate::api::{
    OAuthGrant, OAuthTokenMaterial, ProviderDriver, ProviderRequestContext, ProviderSecret,
};

fn request_context(
    headers: &HeaderMap,
    operation: ProtocolOperation,
    oauth: bool,
) -> ProviderRequestContext<'_> {
    ProviderRequestContext {
        ingress_dialect: operation.dialect(),
        upstream_operation: operation,
        upstream_model: "model",
        client_headers: headers,
        oauth,
        allow_credential_bound: true,
        allow_session_replay: true,
        allow_turn_state: false,
    }
}

#[test]
fn gates_responses_speed_tier_and_oauth_request_costs_without_body_parsing() {
    let driver = CodexDriver::new();
    let headers = HeaderMap::new();
    let responses = request_context(&headers, ProtocolOperation::Responses, true);

    assert_eq!(
        driver.request_speed_tier(responses, Some(RequestSpeedTier::Fast)),
        Some(RequestSpeedTier::Fast)
    );
    assert_eq!(
        driver.response_speed_tier(ProtocolOperation::Responses, Some(RequestSpeedTier::Fast),),
        Some(RequestSpeedTier::Fast)
    );
    assert_eq!(
        driver.oauth_request_quota_cost_unit(responses),
        Some(QuotaCostUnit::CodexCredits)
    );
    assert_eq!(
        driver.request_speed_tier(
            request_context(&headers, ProtocolOperation::ChatCompletions, false),
            Some(RequestSpeedTier::Fast),
        ),
        None
    );
    assert_eq!(
        driver.response_speed_tier(
            ProtocolOperation::ChatCompletions,
            Some(RequestSpeedTier::Fast),
        ),
        None
    );
    assert_eq!(
        driver.oauth_request_quota_cost_unit(request_context(
            &headers,
            ProtocolOperation::Responses,
            false,
        )),
        None
    );
}

#[test]
fn builds_responses_paths_and_bearer_authentication() {
    let driver = CodexDriver::new();
    let base = ProviderBaseUrl::parse("https://api.example.com/v1").expect("base URL");
    assert_eq!(
        driver
            .endpoint_plan(&base, ProtocolOperation::ResponsesCompact)
            .expect("endpoint")
            .url
            .as_str(),
        "https://api.example.com/v1/responses/compact"
    );
    assert_eq!(
        driver
            .endpoint_plan(&base, ProtocolOperation::ImagesGenerations)
            .expect("image generation endpoint")
            .url
            .as_str(),
        "https://api.example.com/v1/images/generations"
    );
    assert_eq!(
        driver
            .endpoint_plan(&base, ProtocolOperation::ImagesEdits)
            .expect("image edit endpoint")
            .url
            .as_str(),
        "https://api.example.com/v1/images/edits"
    );
    assert_eq!(
        driver
            .credential_test_plan(&base)
            .expect("credential test endpoint")
            .url
            .as_str(),
        "https://api.example.com/v1/models"
    );
    let headers = driver
        .credential_headers(&base, &ProviderSecret::new("sk-codex"))
        .expect("headers");
    assert_eq!(headers.headers[AUTHORIZATION], "Bearer sk-codex");
    assert!(!format!("{headers:?}").contains("sk-codex"));
    assert!(
        driver
            .capabilities()
            .transport_modes
            .contains(&TransportMode::Sse)
    );
    assert!(
        driver
            .capabilities()
            .protocols
            .contains(&ProtocolDialect::OpenAiImages)
    );
    assert!(!driver.oauth_supports_operation(ProtocolOperation::ImagesGenerations));
    assert!(!driver.oauth_supports_operation(ProtocolOperation::ImagesEdits));
}

#[test]
fn builds_pkce_authorization_and_token_requests() {
    let driver = CodexDriver::new();
    let authorization = driver
        .oauth_authorization_url("state-value", "challenge-value")
        .expect("authorization URL");
    let query: std::collections::HashMap<_, _> = authorization.query_pairs().into_owned().collect();
    assert_eq!(query.get("state").map(String::as_str), Some("state-value"));
    assert_eq!(
        query.get("code_challenge").map(String::as_str),
        Some("challenge-value")
    );
    assert_eq!(
        query.get("redirect_uri").map(String::as_str),
        Some("http://localhost:1455/auth/callback")
    );

    let plan = driver
        .oauth_token_request(
            OAuthGrant::AuthorizationCode,
            "authorization-code",
            None,
            Some("verifier-value"),
        )
        .expect("token request");
    assert_eq!(
        plan.headers[CONTENT_TYPE],
        "application/x-www-form-urlencoded"
    );
    let form: std::collections::HashMap<_, _> = url::form_urlencoded::parse(&plan.body)
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
    assert!(!format!("{plan:?}").contains("verifier-value"));

    let refresh = driver
        .oauth_token_request(OAuthGrant::RefreshToken, "refresh-secret", None, None)
        .expect("refresh request");
    assert_eq!(refresh.headers[CONTENT_TYPE], "application/json");
    let body = serde_json::from_slice::<serde_json::Value>(&refresh.body).expect("refresh JSON");
    assert_eq!(body["client_id"], "app_EMoamEEZ73f0CkXaXp7hrann");
    assert_eq!(body["grant_type"], "refresh_token");
    assert_eq!(body["refresh_token"], "refresh-secret");
    assert!(!format!("{refresh:?}").contains("refresh-secret"));
}

#[test]
fn classifies_declared_codex_refresh_token_failures_as_permanent() {
    let driver = CodexDriver::new();
    for (code, expected) in [
        (
            "invalid_grant",
            crate::api::OAuthRefreshRejection::InvalidGrant,
        ),
        (
            "refresh_token_expired",
            crate::api::OAuthRefreshRejection::RefreshTokenExpired,
        ),
        (
            "refresh_token_reused",
            crate::api::OAuthRefreshRejection::RefreshTokenReused,
        ),
        (
            "refresh_token_invalidated",
            crate::api::OAuthRefreshRejection::RefreshTokenInvalidated,
        ),
    ] {
        let body = serde_json::json!({ "error": { "code": code } });
        assert_eq!(
            driver.classify_oauth_refresh_rejection(
                StatusCode::UNAUTHORIZED,
                body.to_string().as_bytes(),
            ),
            expected
        );
    }

    assert_eq!(
        driver.classify_oauth_refresh_rejection(
            StatusCode::UNAUTHORIZED,
            br#"{"error":{"code":"unknown"}}"#,
        ),
        crate::api::OAuthRefreshRejection::Unverified
    );
}

#[test]
fn refresh_preserves_omitted_codex_identity_fields() {
    let driver = CodexDriver::new();
    let previous = OAuthTokenMaterial::new(
        ProviderKind::Codex,
        "old-access".into(),
        Some("old-refresh".into()),
        Some("old-id-token".into()),
        Some(42),
        Some("account-123".into()),
        Some("person@example.com".into()),
    )
    .expect("previous token");
    let refreshed = driver
        .parse_oauth_refresh_response(
            br#"{"access_token":"new-access","expires_in":3600}"#,
            &previous,
        )
        .expect("refreshed token");

    assert_eq!(refreshed.access_token(), "new-access");
    assert_eq!(refreshed.refresh_token(), Some("old-refresh"));
    assert_eq!(refreshed.id_token(), Some("old-id-token"));
    assert!(refreshed.expires_at().is_some_and(|expiry| expiry > 42));
    assert_eq!(refreshed.account_id(), Some("account-123"));
    assert_eq!(refreshed.email(), Some("person@example.com"));
}

#[test]
fn parses_codex_token_claims_without_logging_token_values() {
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
        br#"{"email":"person@example.com","https://api.openai.com/auth":{"chatgpt_account_id":"account-123","chatgpt_plan_type":"plus"}}"#,
    );
    let id_token = format!("header.{payload}.signature");
    let driver = CodexDriver::new();
    let token = driver
        .parse_oauth_token_response(
            serde_json::json!({
                "access_token": "access-secret",
                "refresh_token": "refresh-secret",
                "id_token": id_token,
                "expires_in": 3600
            })
            .to_string()
            .as_bytes(),
        )
        .expect("token response");
    assert_eq!(token.account_id(), Some("account-123"));
    assert_eq!(token.email(), Some("person@example.com"));
    assert!(driver.oauth_principal_identity(&token).is_some());
    let identity_debug = format!("{:?}", driver.oauth_principal_identity(&token));
    assert!(!identity_debug.contains("account-123"));
    assert!(!identity_debug.contains("person@example.com"));
    assert!(!format!("{token:?}").contains("person@example.com"));
    let profile = driver
        .oauth_routing_profile(&token)
        .expect("OAuth routing profile");
    assert_eq!(
        profile.base_url().as_str(),
        "https://chatgpt.com/backend-api/codex"
    );
    assert_eq!(profile.protocol_dialect(), ProtocolDialect::OpenAiResponses);
    assert_eq!(
        driver
            .endpoint_plan(profile.base_url(), ProtocolOperation::Responses)
            .expect("OAuth endpoint")
            .url
            .as_str(),
        "https://chatgpt.com/backend-api/codex/responses"
    );
}

#[test]
fn codex_principal_identity_uses_member_claim_not_workspace_alone() {
    fn token(workspace: &str, member: &str) -> OAuthTokenMaterial {
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
            serde_json::json!({
                "https://api.openai.com/auth": {
                    "chatgpt_account_id": workspace,
                    "chatgpt_user_id": member,
                }
            })
            .to_string(),
        );
        OAuthTokenMaterial::new(
            ProviderKind::Codex,
            "access-token".into(),
            None,
            Some(format!("header.{payload}.signature")),
            None,
            Some(workspace.into()),
            None,
        )
        .expect("token")
    }

    let driver = CodexDriver::new();
    let first = token("workspace-a", "member-a");
    let same_member = token("workspace-a", "member-a");
    let other_member = token("workspace-a", "member-b");
    let other_workspace = token("workspace-b", "member-a");
    assert_eq!(
        driver.oauth_principal_identity(&first),
        driver.oauth_principal_identity(&same_member)
    );
    assert_ne!(
        driver.oauth_principal_identity(&first),
        driver.oauth_principal_identity(&other_member)
    );
    assert_ne!(
        driver.oauth_principal_identity(&first),
        driver.oauth_principal_identity(&other_workspace)
    );

    let legacy_payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
        br#"{"https://api.openai.com/auth":{"chatgpt_account_id":"workspace-a","user_id":"member-a"}}"#,
    );
    let legacy = OAuthTokenMaterial::new(
        ProviderKind::Codex,
        "access-token-legacy".into(),
        None,
        Some(format!("header.{legacy_payload}.signature")),
        None,
        Some("workspace-a".into()),
        None,
    )
    .expect("legacy token");
    assert_eq!(
        driver.oauth_principal_identity(&first),
        driver.oauth_principal_identity(&legacy)
    );
}

#[test]
fn missing_codex_plan_does_not_create_a_local_catalog() {
    let driver = CodexDriver::new();
    let token = driver
        .parse_oauth_token_response(br#"{"access_token":"access-secret","expires_in":3600}"#)
        .expect("token response");
    let profile = driver
        .oauth_routing_profile(&token)
        .expect("OAuth routing profile");

    assert_eq!(
        profile.base_url().as_str(),
        "https://chatgpt.com/backend-api/codex"
    );
}

#[test]
fn builds_codex_oauth_headers_from_token_response() {
    let driver = CodexDriver::new();
    let token = driver
        .parse_oauth_token_response(
            br#"{"access_token":"oauth-secret","account_id":"account-123","expires_in":3600}"#,
        )
        .expect("OAuth token response");
    let headers = driver
        .oauth_credential_headers(&token, &http::HeaderMap::new())
        .expect("OAuth headers");

    assert_eq!(headers.headers[AUTHORIZATION], "Bearer oauth-secret");
    assert_eq!(headers.headers["chatgpt-account-id"], "account-123");
    assert!(!headers.headers.contains_key("originator"));
    let identity = driver
        .prepare_request_headers(ProviderRequestContext {
            ingress_dialect: ProtocolDialect::OpenAiResponses,
            upstream_operation: ProtocolOperation::Responses,
            upstream_model: "gpt",
            client_headers: &http::HeaderMap::new(),
            oauth: true,
            allow_credential_bound: true,
            allow_session_replay: true,
            allow_turn_state: false,
        })
        .expect("identity headers");
    assert_eq!(identity["originator"], "codex_cli_rs");
    assert!(!format!("{headers:?}").contains("oauth-secret"));
}
