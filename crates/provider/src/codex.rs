use any2api_domain::{
    CredentialKind, ProtocolDialect, ProtocolOperation, ProviderKind, TransportMode,
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use http::{HeaderMap, HeaderValue, Method, header};
use serde::Deserialize;
use url::Url;

use crate::{
    ProviderError, ProviderSecret,
    api::{
        CapabilitySet, CredentialHeaders, EndpointPlan, OAuthGrant, OAuthRequestPlan,
        OAuthTokenMaterial, ProviderDriver, UpstreamResponseMeta,
    },
    api_key, codex_error,
    oauth::form_headers,
};

const OAUTH_AUTHORIZE_URL: &str = "https://auth.openai.com/oauth/authorize";
const OAUTH_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const OAUTH_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const OAUTH_REDIRECT_URI: &str = "http://localhost:1455/auth/callback";

#[derive(Debug)]
pub struct CodexDriver {
    capabilities: CapabilitySet,
}

impl Default for CodexDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl CodexDriver {
    #[must_use]
    pub fn new() -> Self {
        Self {
            capabilities: CapabilitySet {
                protocols: [
                    ProtocolDialect::OpenAiResponses,
                    ProtocolDialect::OpenAiChatCompletions,
                ]
                .into_iter()
                .collect(),
                transport_modes: [TransportMode::Json, TransportMode::Sse]
                    .into_iter()
                    .collect(),
                credential_kinds: [CredentialKind::ApiKey].into_iter().collect(),
            },
        }
    }
}

impl ProviderDriver for CodexDriver {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Codex
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
            ProtocolOperation::Responses
                | ProtocolOperation::ResponsesCompact
                | ProtocolOperation::ChatCompletions
        ) {
            return Err(ProviderError::InvalidEndpoint(
                "operation is not supported by Codex".into(),
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
        let value = format!("Bearer {}", secret.expose());
        let authorization = HeaderValue::from_str(&value)
            .map_err(|_| ProviderError::InvalidCredential("invalid API Key header".into()))?;
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, authorization);
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
            .append_pair("client_id", OAUTH_CLIENT_ID)
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", OAUTH_REDIRECT_URI)
            .append_pair("scope", "openid profile email offline_access")
            .append_pair("state", state)
            .append_pair("code_challenge", code_challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("prompt", "login")
            .append_pair("id_token_add_organizations", "true")
            .append_pair("codex_cli_simplified_flow", "true");
        Ok(url)
    }

    fn oauth_token_request(
        &self,
        grant: OAuthGrant,
        code: &str,
        _state: Option<&str>,
        code_verifier: Option<&str>,
    ) -> Result<OAuthRequestPlan, ProviderError> {
        match grant {
            OAuthGrant::AuthorizationCode => {
                let verifier = code_verifier.ok_or_else(|| {
                    ProviderError::InvalidCredential("OAuth code verifier is required".into())
                })?;
                let mut form = url::form_urlencoded::Serializer::new(String::new());
                form.append_pair("client_id", OAUTH_CLIENT_ID)
                    .append_pair("grant_type", "authorization_code")
                    .append_pair("code", code)
                    .append_pair("redirect_uri", OAUTH_REDIRECT_URI)
                    .append_pair("code_verifier", verifier);
                Ok(OAuthRequestPlan {
                    method: Method::POST,
                    url: Url::parse(OAUTH_TOKEN_URL)
                        .map_err(|error| ProviderError::InvalidEndpoint(error.to_string()))?,
                    headers: form_headers(),
                    body: form.finish().into_bytes(),
                })
            }
        }
    }

    fn parse_oauth_token(&self, body: &[u8]) -> Result<OAuthTokenMaterial, ProviderError> {
        let response = serde_json::from_slice::<CodexOAuthResponse>(body).map_err(|_| {
            ProviderError::InvalidResponse("Codex OAuth response is invalid".into())
        })?;
        let (account_id, email) = decode_codex_claims(response.id_token.as_deref());
        OAuthTokenMaterial::new(
            ProviderKind::Codex,
            response.access_token,
            response.refresh_token,
            response.id_token,
            response
                .expires_in
                .map(|seconds| unix_now().saturating_add(seconds)),
            account_id,
            email,
        )
    }

    fn classify_error(
        &self,
        _operation: ProtocolOperation,
        meta: &UpstreamResponseMeta,
        bounded_body: &[u8],
    ) -> any2api_domain::UpstreamErrorClassification {
        codex_error::classify(meta, bounded_body)
    }
}

#[derive(Deserialize)]
struct CodexOAuthResponse {
    access_token: String,
    refresh_token: Option<String>,
    id_token: Option<String>,
    expires_in: Option<i64>,
}

fn decode_codex_claims(id_token: Option<&str>) -> (Option<String>, Option<String>) {
    let Some(payload) = id_token.and_then(|token| token.split('.').nth(1)) else {
        return (None, None);
    };
    let Ok(bytes) = URL_SAFE_NO_PAD.decode(payload) else {
        return (None, None);
    };
    #[derive(Deserialize)]
    struct Claims {
        email: Option<String>,
        #[serde(rename = "https://api.openai.com/auth")]
        auth: Option<AuthClaims>,
    }
    #[derive(Deserialize)]
    struct AuthClaims {
        chatgpt_account_id: Option<String>,
    }
    let Ok(claims) = serde_json::from_slice::<Claims>(&bytes) else {
        return (None, None);
    };
    (
        claims.auth.and_then(|auth| auth.chatgpt_account_id),
        claims.email,
    )
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use any2api_domain::{ProtocolOperation, ProviderBaseUrl, TransportMode};
    use base64::Engine as _;
    use http::{header::AUTHORIZATION, header::CONTENT_TYPE};

    use super::CodexDriver;
    use crate::{OAuthGrant, ProviderSecret, api::ProviderDriver};

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
                .credential_test_plan(&base)
                .expect("credential test endpoint")
                .url
                .as_str(),
            "https://api.example.com/v1/models"
        );
        let headers = driver
            .credential_headers(&ProviderSecret::new(1, "sk-codex"))
            .expect("headers");
        assert_eq!(headers.headers[AUTHORIZATION], "Bearer sk-codex");
        assert!(!format!("{headers:?}").contains("sk-codex"));
        assert!(
            driver
                .capabilities()
                .transport_modes
                .contains(&TransportMode::Sse)
        );
    }

    #[test]
    fn builds_pkce_authorization_and_token_requests() {
        let driver = CodexDriver::new();
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
    }

    #[test]
    fn parses_codex_token_claims_without_logging_token_values() {
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
            br#"{"email":"person@example.com","https://api.openai.com/auth":{"chatgpt_account_id":"account-123"}}"#,
        );
        let id_token = format!("header.{payload}.signature");
        let driver = CodexDriver::new();
        let token = driver
            .parse_oauth_token(
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
        assert!(!format!("{token:?}").contains("person@example.com"));
    }
}
