use any2api_domain::{
    CredentialKind, ProtocolDialect, ProtocolOperation, ProviderKind, TransportMode,
};
use http::{HeaderMap, HeaderValue};
use url::Url;

use crate::{
    ProviderError, ProviderSecret,
    api::{
        CapabilitySet, CredentialHeaders, EndpointPlan, OAuthGrant, OAuthRequestPlan,
        OAuthRoutingProfile, OAuthTokenMaterial, ProviderDriver, UpstreamResponseMeta,
    },
    api_key, claude_error, claude_oauth,
};

#[derive(Debug)]
pub struct ClaudeDriver {
    capabilities: CapabilitySet,
}

impl Default for ClaudeDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaudeDriver {
    #[must_use]
    pub fn new() -> Self {
        Self {
            capabilities: CapabilitySet {
                protocols: [ProtocolDialect::AnthropicMessages].into_iter().collect(),
                transport_modes: [TransportMode::Json, TransportMode::Sse]
                    .into_iter()
                    .collect(),
                credential_kinds: [CredentialKind::ApiKey].into_iter().collect(),
            },
        }
    }
}

impl ProviderDriver for ClaudeDriver {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Claude
    }

    fn capabilities(&self) -> &CapabilitySet {
        &self.capabilities
    }

    fn validate_credential(&self, secret: &ProviderSecret) -> Result<(), ProviderError> {
        api_key::validate_secret(secret)
    }

    fn endpoint_plan(
        &self,
        base_url: &any2api_domain::ProviderBaseUrl,
        operation: ProtocolOperation,
    ) -> Result<EndpointPlan, ProviderError> {
        if !matches!(
            operation,
            ProtocolOperation::Messages | ProtocolOperation::MessagesCountTokens
        ) {
            return Err(ProviderError::InvalidEndpoint(
                "operation is not supported by Claude".into(),
            ));
        }
        Ok(EndpointPlan {
            url: api_key::endpoint_url(base_url, operation)?,
        })
    }

    fn credential_headers(
        &self,
        secret: &ProviderSecret,
    ) -> Result<CredentialHeaders, ProviderError> {
        self.validate_credential(secret)?;
        let api_key = HeaderValue::from_str(secret.expose())
            .map_err(|_| ProviderError::InvalidCredential("invalid API Key header".into()))?;
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", api_key);
        headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
        Ok(CredentialHeaders { headers })
    }

    fn credential_test_plan(
        &self,
        base_url: &any2api_domain::ProviderBaseUrl,
    ) -> Result<EndpointPlan, ProviderError> {
        Ok(EndpointPlan {
            url: api_key::credential_test_url(base_url)?,
        })
    }

    fn parse_model_catalog(&self, bounded_body: &[u8]) -> Result<Vec<String>, ProviderError> {
        api_key::parse_model_catalog(bounded_body)
    }

    fn oauth_redirect_uri(&self) -> Option<&'static str> {
        Some(claude_oauth::redirect_uri())
    }

    fn oauth_authorization_url(
        &self,
        state: &str,
        code_challenge: &str,
    ) -> Result<Url, ProviderError> {
        claude_oauth::authorization_url(state, code_challenge)
    }

    fn oauth_token_request(
        &self,
        grant: OAuthGrant,
        code: &str,
        state: Option<&str>,
        code_verifier: Option<&str>,
    ) -> Result<OAuthRequestPlan, ProviderError> {
        claude_oauth::token_request(grant, code, state, code_verifier)
    }

    fn parse_oauth_token(&self, body: &[u8]) -> Result<OAuthTokenMaterial, ProviderError> {
        claude_oauth::parse_token(body)
    }

    fn oauth_routing_profile(
        &self,
        _token: &OAuthTokenMaterial,
    ) -> Result<OAuthRoutingProfile, ProviderError> {
        claude_oauth::routing_profile()
    }

    fn oauth_credential_headers(
        &self,
        token: &OAuthTokenMaterial,
        forwarded: &HeaderMap,
    ) -> Result<CredentialHeaders, ProviderError> {
        claude_oauth::credential_headers(token, forwarded)
    }

    fn classify_error(
        &self,
        operation: ProtocolOperation,
        meta: &UpstreamResponseMeta,
        bounded_body: &[u8],
    ) -> any2api_domain::UpstreamErrorClassification {
        claude_error::classify(operation, meta, bounded_body)
    }
}

#[cfg(test)]
mod tests {
    use any2api_domain::{
        ProtocolDialect, ProtocolOperation, ProviderBaseUrl, TransportMode, UpstreamErrorKind,
    };
    use http::{HeaderMap, StatusCode, header::CONTENT_TYPE};

    use super::ClaudeDriver;
    use crate::{
        OAuthGrant, ProviderSecret,
        api::{ProviderDriver, UpstreamResponseMeta},
    };

    #[test]
    fn builds_messages_paths_and_anthropic_headers() {
        let driver = ClaudeDriver::new();
        let base = ProviderBaseUrl::parse("https://api.example.com/v1").expect("base URL");
        assert_eq!(
            driver
                .endpoint_plan(&base, ProtocolOperation::MessagesCountTokens)
                .expect("endpoint")
                .url
                .as_str(),
            "https://api.example.com/v1/messages/count_tokens"
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
            .credential_headers(&ProviderSecret::new(1, "sk-claude"))
            .expect("headers");
        assert_eq!(headers.headers["x-api-key"], "sk-claude");
        assert_eq!(headers.headers["anthropic-version"], "2023-06-01");
        assert!(
            driver
                .capabilities()
                .transport_modes
                .contains(&TransportMode::Sse)
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
                .kind(),
            UpstreamErrorKind::OperationUnavailable
        );
        assert_eq!(
            driver
                .classify_error(ProtocolOperation::Messages, &response, b"{}")
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
        let query: std::collections::HashMap<_, _> =
            authorization.query_pairs().into_owned().collect();
        assert_eq!(query.get("state").map(String::as_str), Some("state-value"));
        assert_eq!(
            query.get("code_challenge").map(String::as_str),
            Some("challenge-value")
        );
        let plan = driver
            .oauth_token_request(
                OAuthGrant::AuthorizationCode,
                "authorization-code",
                Some("state-value"),
                Some("verifier-value"),
            )
            .expect("token request");
        assert_eq!(plan.headers[CONTENT_TYPE], "application/json");
        let body: serde_json::Value = serde_json::from_slice(&plan.body).expect("token JSON");
        assert_eq!(body["code"], "authorization-code");
        assert_eq!(body["state"], "state-value");
        assert_eq!(body["code_verifier"], "verifier-value");
        assert!(!format!("{plan:?}").contains("verifier-value"));

        let refresh = driver
            .oauth_token_request(OAuthGrant::RefreshToken, "refresh-secret", None, None)
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
            .parse_oauth_token(
                br#"{"access_token":"access-secret","refresh_token":"refresh-secret","account":{"email_address":"claude@example.com"}}"#,
            )
            .expect("token response");
        assert_eq!(token.email(), Some("claude@example.com"));
        assert!(!format!("{token:?}").contains("claude@example.com"));
        let profile = driver
            .oauth_routing_profile(&token)
            .expect("OAuth routing profile");
        assert_eq!(profile.base_url().as_str(), "https://api.anthropic.com/v1");
        assert_eq!(
            profile.protocol_dialect(),
            ProtocolDialect::AnthropicMessages
        );
        assert_eq!(profile.models().len(), 14);
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
            .parse_oauth_token(br#"{"type":"claude","access_token":"oauth-secret"}"#)
            .expect("stored OAuth document");
        let mut forwarded = HeaderMap::new();
        forwarded.insert(
            "anthropic-beta",
            "custom-beta".parse().expect("beta header"),
        );
        let headers = driver
            .oauth_credential_headers(&token, &forwarded)
            .expect("OAuth headers");

        assert_eq!(headers.headers["authorization"], "Bearer oauth-secret");
        assert_eq!(headers.headers["anthropic-version"], "2023-06-01");
        assert_eq!(
            headers.headers["anthropic-beta"],
            "custom-beta,oauth-2025-04-20"
        );
        assert!(!format!("{headers:?}").contains("oauth-secret"));
    }
}
