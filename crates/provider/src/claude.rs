use any2api_domain::{
    CredentialKind, ProtocolDialect, ProtocolOperation, ProviderKind, TransportMode,
};
use http::{HeaderMap, HeaderValue, Method, header};
use serde::Deserialize;
use url::Url;

use crate::{
    ProviderError, ProviderSecret,
    api::{CapabilitySet, CredentialHeaders, EndpointPlan, ProviderDriver, UpstreamResponseMeta},
    api_key, claude_error,
    oauth::{OAuthGrant, OAuthRequestPlan, OAuthTokenMaterial, json_headers, validate_oauth_kind},
};

const OAUTH_AUTHORIZE_URL: &str = "https://claude.ai/oauth/authorize";
const OAUTH_TOKEN_URL: &str = "https://api.anthropic.com/v1/oauth/token";
const OAUTH_CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const OAUTH_REDIRECT_URI: &str = "http://localhost:54545/callback";

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
                credential_kinds: [CredentialKind::ApiKey, CredentialKind::OAuth2]
                    .into_iter()
                    .collect(),
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
        credential_kind: CredentialKind,
        secret: &ProviderSecret,
    ) -> Result<CredentialHeaders, ProviderError> {
        let mut headers = HeaderMap::new();
        match credential_kind {
            CredentialKind::ApiKey => {
                self.validate_credential(secret)?;
                let api_key = HeaderValue::from_str(secret.expose()).map_err(|_| {
                    ProviderError::InvalidCredential("invalid API Key header".into())
                })?;
                headers.insert("x-api-key", api_key);
            }
            CredentialKind::OAuth2 => {
                validate_oauth_kind(credential_kind)?;
                let token = OAuthTokenMaterial::from_secret(ProviderKind::Claude, secret)?;
                let value = HeaderValue::from_str(&format!("Bearer {}", token.access_token()))
                    .map_err(|_| ProviderError::InvalidCredential("invalid OAuth token".into()))?;
                headers.insert(header::AUTHORIZATION, value);
                headers.insert(
                    "anthropic-beta",
                    HeaderValue::from_static("oauth-2025-04-20"),
                );
            }
        }
        headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
        Ok(CredentialHeaders { headers })
    }

    fn credential_test_plan(
        &self,
        base_url: &any2api_domain::ProviderBaseUrl,
        _credential_kind: CredentialKind,
    ) -> Result<EndpointPlan, ProviderError> {
        Ok(EndpointPlan {
            url: api_key::credential_test_url(base_url)?,
        })
    }

    fn parse_model_catalog(
        &self,
        _credential_kind: CredentialKind,
        bounded_body: &[u8],
    ) -> Result<Vec<String>, ProviderError> {
        api_key::parse_model_catalog(bounded_body)
    }

    fn oauth_redirect_uri(&self) -> Option<&'static str> {
        Some(OAUTH_REDIRECT_URI)
    }

    fn oauth_authorization_url(
        &self,
        state: &str,
        code_challenge: &str,
    ) -> Result<Url, ProviderError> {
        let mut url = Url::parse(OAUTH_AUTHORIZE_URL)
            .map_err(|error| ProviderError::InvalidEndpoint(error.to_string()))?;
        url.query_pairs_mut()
            .append_pair("code", "true")
            .append_pair("client_id", OAUTH_CLIENT_ID)
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", OAUTH_REDIRECT_URI)
            .append_pair(
                "scope",
                "user:profile user:inference user:sessions:claude_code user:mcp_servers user:file_upload",
            )
            .append_pair("code_challenge", code_challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("state", state);
        Ok(url)
    }

    fn oauth_token_request(
        &self,
        grant: OAuthGrant,
        code_or_refresh_token: &str,
        state: Option<&str>,
        code_verifier: Option<&str>,
    ) -> Result<OAuthRequestPlan, ProviderError> {
        let body = match grant {
            OAuthGrant::AuthorizationCode => serde_json::json!({
                "code": code_or_refresh_token,
                "state": state.unwrap_or_default(),
                "grant_type": "authorization_code",
                "client_id": OAUTH_CLIENT_ID,
                "redirect_uri": OAUTH_REDIRECT_URI,
                "code_verifier": code_verifier.ok_or_else(|| ProviderError::InvalidCredential("OAuth code verifier is required".into()))?,
            }),
            OAuthGrant::RefreshToken => serde_json::json!({
                "client_id": OAUTH_CLIENT_ID,
                "grant_type": "refresh_token",
                "refresh_token": code_or_refresh_token,
            }),
        };
        Ok(OAuthRequestPlan {
            method: Method::POST,
            url: Url::parse(OAUTH_TOKEN_URL)
                .map_err(|error| ProviderError::InvalidEndpoint(error.to_string()))?,
            headers: json_headers(),
            body: serde_json::to_vec(&body).map_err(|_| {
                ProviderError::InvalidResponse("OAuth request serialization failed".into())
            })?,
        })
    }

    fn parse_oauth_token(
        &self,
        body: &[u8],
        previous: Option<&OAuthTokenMaterial>,
    ) -> Result<OAuthTokenMaterial, ProviderError> {
        let response = serde_json::from_slice::<ClaudeOAuthResponse>(body).map_err(|_| {
            ProviderError::InvalidResponse("Claude OAuth response is invalid".into())
        })?;
        OAuthTokenMaterial::new(
            ProviderKind::Claude,
            response.access_token,
            response.refresh_token,
            None,
            response
                .expires_in
                .map(|seconds| unix_now().saturating_add(seconds)),
            response
                .account
                .as_ref()
                .and_then(|account| account.uuid.clone()),
            response
                .account
                .as_ref()
                .and_then(|account| account.email_address.clone()),
            response
                .organization
                .as_ref()
                .and_then(|org| org.uuid.clone()),
            Some(OAUTH_CLIENT_ID.to_owned()),
        )
        .map(|token| previous.map_or(token.clone(), |previous| token.with_fallback_from(previous)))
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
        CredentialKind, ProtocolOperation, ProviderBaseUrl, TransportMode, UpstreamErrorKind,
    };
    use http::{HeaderMap, StatusCode};

    use super::ClaudeDriver;
    use crate::{
        ProviderSecret,
        api::{OAuthGrant, ProviderDriver, UpstreamResponseMeta},
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
                .credential_test_plan(&base, CredentialKind::ApiKey)
                .expect("credential test endpoint")
                .url
                .as_str(),
            "https://api.example.com/v1/models"
        );
        let headers = driver
            .credential_headers(CredentialKind::ApiKey, &ProviderSecret::new(1, "sk-claude"))
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
    fn builds_pkce_login_and_oauth_bearer_headers() {
        let driver = ClaudeDriver::new();
        let authorization = driver
            .oauth_authorization_url("state-value", "challenge-value")
            .expect("authorization URL");
        let query = authorization
            .query_pairs()
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(
            query.get("state").map(|value| value.as_ref()),
            Some("state-value")
        );
        assert_eq!(
            query.get("redirect_uri").map(|value| value.as_ref()),
            Some("http://localhost:54545/callback")
        );
        let request = driver
            .oauth_token_request(
                OAuthGrant::AuthorizationCode,
                "authorization-code",
                Some("state-value"),
                Some("verifier-value"),
            )
            .expect("token request");
        let request_body: serde_json::Value =
            serde_json::from_slice(&request.body).expect("JSON request");
        assert_eq!(request_body["state"], "state-value");
        assert_eq!(request_body["code_verifier"], "verifier-value");
        assert!(!format!("{request:?}").contains("authorization-code"));

        let token = driver
            .parse_oauth_token(
                br#"{"access_token":"oauth-access","refresh_token":"oauth-refresh","expires_in":3600,"organization":{"uuid":"org-id"},"account":{"uuid":"account-id","email_address":"owner@example.com"}}"#,
                None,
            )
            .expect("OAuth token");
        let headers = driver
            .credential_headers(
                CredentialKind::OAuth2,
                &token.to_secret().expect("typed secret"),
            )
            .expect("OAuth headers");
        assert_eq!(
            headers.headers[http::header::AUTHORIZATION],
            "Bearer oauth-access"
        );
        assert_eq!(headers.headers["anthropic-beta"], "oauth-2025-04-20");
        assert_eq!(token.organization_id(), Some("org-id"));
        assert_eq!(token.email(), Some("owner@example.com"));
    }
}

#[derive(Deserialize)]
struct ClaudeOAuthResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
    organization: Option<ClaudeOrganization>,
    account: Option<ClaudeAccount>,
}

#[derive(Deserialize)]
struct ClaudeOrganization {
    uuid: Option<String>,
}

#[derive(Deserialize)]
struct ClaudeAccount {
    uuid: Option<String>,
    email_address: Option<String>,
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or(i64::MAX)
}
