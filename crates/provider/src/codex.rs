use any2api_domain::{
    CredentialKind, ProtocolDialect, ProtocolOperation, ProviderKind, TransportMode,
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use http::{HeaderMap, HeaderValue, Method, header};
use serde::Deserialize;
use url::Url;

use crate::{
    ProviderError, ProviderSecret,
    api::{CapabilitySet, CredentialHeaders, EndpointPlan, ProviderDriver, UpstreamResponseMeta},
    api_key, codex_error,
    oauth::{OAuthGrant, OAuthRequestPlan, OAuthTokenMaterial, form_headers, validate_oauth_kind},
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
                protocols: [ProtocolDialect::OpenAiResponses].into_iter().collect(),
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
            ProtocolOperation::Responses | ProtocolOperation::ResponsesCompact
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
        credential_kind: CredentialKind,
        secret: &ProviderSecret,
    ) -> Result<CredentialHeaders, ProviderError> {
        let (value, account_id) = match credential_kind {
            CredentialKind::ApiKey => {
                self.validate_credential(secret)?;
                (secret.expose().to_owned(), None)
            }
            CredentialKind::OAuth2 => {
                validate_oauth_kind(credential_kind)?;
                let token = OAuthTokenMaterial::from_secret(ProviderKind::Codex, secret)?;
                (
                    token.access_token().to_owned(),
                    token.account_id().map(str::to_owned),
                )
            }
        };
        let value = format!("Bearer {value}");
        let authorization = HeaderValue::from_str(&value)
            .map_err(|_| ProviderError::InvalidCredential("invalid API Key header".into()))?;
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, authorization);
        if let Some(account_id) = account_id {
            headers.insert(
                "Chatgpt-Account-Id",
                HeaderValue::from_str(&account_id).map_err(|_| {
                    ProviderError::InvalidCredential("invalid ChatGPT account id".into())
                })?,
            );
        }
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
        credential_kind: CredentialKind,
        bounded_body: &[u8],
    ) -> Result<Vec<String>, ProviderError> {
        if credential_kind == CredentialKind::OAuth2 {
            return parse_oauth_model_catalog(bounded_body);
        }
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
        code_or_refresh_token: &str,
        _state: Option<&str>,
        code_verifier: Option<&str>,
    ) -> Result<OAuthRequestPlan, ProviderError> {
        let mut form = url::form_urlencoded::Serializer::new(String::new());
        form.append_pair("client_id", OAUTH_CLIENT_ID);
        match grant {
            OAuthGrant::AuthorizationCode => {
                let verifier = code_verifier.ok_or_else(|| {
                    ProviderError::InvalidCredential("OAuth code verifier is required".into())
                })?;
                form.append_pair("grant_type", "authorization_code")
                    .append_pair("code", code_or_refresh_token)
                    .append_pair("redirect_uri", OAUTH_REDIRECT_URI)
                    .append_pair("code_verifier", verifier);
            }
            OAuthGrant::RefreshToken => {
                form.append_pair("grant_type", "refresh_token")
                    .append_pair("refresh_token", code_or_refresh_token)
                    .append_pair("scope", "openid profile email");
            }
        }
        Ok(OAuthRequestPlan {
            method: Method::POST,
            url: Url::parse(OAUTH_TOKEN_URL)
                .map_err(|error| ProviderError::InvalidEndpoint(error.to_string()))?,
            headers: form_headers(),
            body: form.finish().into_bytes(),
        })
    }

    fn parse_oauth_token(
        &self,
        body: &[u8],
        previous: Option<&OAuthTokenMaterial>,
    ) -> Result<OAuthTokenMaterial, ProviderError> {
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
            None,
            Some(OAUTH_CLIENT_ID.to_owned()),
        )
        .map(|token| previous.map_or(token.clone(), |previous| token.with_fallback_from(previous)))
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

#[cfg(test)]
mod tests {
    use any2api_domain::{CredentialKind, ProtocolOperation, ProviderBaseUrl, TransportMode};
    use base64::Engine as _;
    use http::header::AUTHORIZATION;

    use super::CodexDriver;
    use crate::{
        ProviderSecret,
        api::{OAuthGrant, ProviderDriver},
    };

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
                .credential_test_plan(&base, CredentialKind::ApiKey)
                .expect("credential test endpoint")
                .url
                .as_str(),
            "https://api.example.com/v1/models"
        );
        let headers = driver
            .credential_headers(CredentialKind::ApiKey, &ProviderSecret::new(1, "sk-codex"))
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
    fn builds_pkce_login_and_parses_oauth_account_material() {
        let driver = CodexDriver::new();
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
            query.get("code_challenge").map(|value| value.as_ref()),
            Some("challenge-value")
        );
        assert_eq!(
            query.get("redirect_uri").map(|value| value.as_ref()),
            Some("http://localhost:1455/auth/callback")
        );

        let request = driver
            .oauth_token_request(
                OAuthGrant::AuthorizationCode,
                "authorization-code",
                Some("state-value"),
                Some("verifier-value"),
            )
            .expect("token request");
        let request_body = String::from_utf8(request.body.clone()).expect("form body");
        assert!(request_body.contains("code=authorization-code"));
        assert!(request_body.contains("code_verifier=verifier-value"));
        assert!(!format!("{request:?}").contains("authorization-code"));

        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
            br#"{"email":"owner@example.com","https://api.openai.com/auth":{"chatgpt_account_id":"account-id"}}"#,
        );
        let body = format!(
            r#"{{"access_token":"oauth-access","refresh_token":"oauth-refresh","id_token":"header.{payload}.signature","expires_in":3600}}"#
        );
        let token = driver
            .parse_oauth_token(body.as_bytes(), None)
            .expect("OAuth token");
        assert_eq!(token.account_id(), Some("account-id"));
        assert_eq!(token.email(), Some("owner@example.com"));
        let secret = token.to_secret().expect("typed secret");
        let headers = driver
            .credential_headers(CredentialKind::OAuth2, &secret)
            .expect("OAuth headers");
        assert_eq!(headers.headers[AUTHORIZATION], "Bearer oauth-access");
        assert_eq!(headers.headers["Chatgpt-Account-Id"], "account-id");
    }
}

#[derive(Deserialize)]
struct CodexOAuthResponse {
    access_token: String,
    refresh_token: Option<String>,
    id_token: Option<String>,
    expires_in: Option<i64>,
}

fn parse_oauth_model_catalog(body: &[u8]) -> Result<Vec<String>, ProviderError> {
    #[derive(Deserialize)]
    struct Catalog {
        models: Vec<Model>,
    }
    #[derive(Deserialize)]
    struct Model {
        slug: Option<String>,
        id: Option<String>,
    }
    let catalog = serde_json::from_slice::<Catalog>(body)
        .map_err(|_| ProviderError::InvalidResponse("Codex model catalog is invalid".into()))?;
    let mut models = std::collections::BTreeSet::new();
    for item in catalog.models {
        let value = item.slug.or(item.id).ok_or_else(|| {
            ProviderError::InvalidResponse("Codex model catalog item has no id".into())
        })?;
        models.insert(
            any2api_domain::UpstreamModelName::new(value)
                .map_err(|error| ProviderError::InvalidResponse(error.to_string()))?
                .as_str()
                .to_owned(),
        );
    }
    Ok(models.into_iter().collect())
}

fn decode_codex_claims(id_token: Option<&str>) -> (Option<String>, Option<String>) {
    let Some(id_token) = id_token else {
        return (None, None);
    };
    let Some(payload) = id_token.split('.').nth(1) else {
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
