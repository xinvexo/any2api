use any2api_domain::{ProtocolDialect, ProviderKind};
use http::{HeaderMap, HeaderValue, Method, header};
use serde::Deserialize;
use url::Url;

use crate::{
    ProviderError,
    api::{OAuthGrant, OAuthRequestPlan, OAuthRoutingProfile, OAuthTokenMaterial},
    oauth::json_headers,
};

const AUTHORIZE_URL: &str = "https://claude.ai/oauth/authorize";
const TOKEN_URL: &str = "https://api.anthropic.com/v1/oauth/token";
const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const REDIRECT_URI: &str = "http://localhost:54545/callback";
const DATA_BASE_URL: &str = "https://api.anthropic.com/v1";
const MODELS: &[&str] = &[
    "claude-3-5-haiku-20241022",
    "claude-3-7-sonnet-20250219",
    "claude-fable-5",
    "claude-haiku-4-5-20251001",
    "claude-opus-4-1-20250805",
    "claude-opus-4-20250514",
    "claude-opus-4-5-20251101",
    "claude-opus-4-6",
    "claude-opus-4-7",
    "claude-opus-4-8",
    "claude-sonnet-4-20250514",
    "claude-sonnet-4-5-20250929",
    "claude-sonnet-4-6",
    "claude-sonnet-5",
];

pub(crate) const fn redirect_uri() -> &'static str {
    REDIRECT_URI
}

pub(crate) fn authorization_url(state: &str, code_challenge: &str) -> Result<Url, ProviderError> {
    let mut url = Url::parse(AUTHORIZE_URL)
        .map_err(|error| ProviderError::InvalidEndpoint(error.to_string()))?;
    url.query_pairs_mut()
        .append_pair("code", "true")
        .append_pair("client_id", CLIENT_ID)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", REDIRECT_URI)
        .append_pair(
            "scope",
            "user:profile user:inference user:sessions:claude_code user:mcp_servers user:file_upload",
        )
        .append_pair("code_challenge", code_challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", state);
    Ok(url)
}

pub(crate) fn token_request(
    grant: OAuthGrant,
    code: &str,
    state: Option<&str>,
    code_verifier: Option<&str>,
) -> Result<OAuthRequestPlan, ProviderError> {
    match grant {
        OAuthGrant::AuthorizationCode => {
            let body = serde_json::json!({
                "code": code,
                "state": state.unwrap_or_default(),
                "grant_type": "authorization_code",
                "client_id": CLIENT_ID,
                "redirect_uri": REDIRECT_URI,
                "code_verifier": code_verifier.ok_or_else(|| ProviderError::InvalidCredential("OAuth code verifier is required".into()))?,
            });
            Ok(OAuthRequestPlan {
                method: Method::POST,
                url: Url::parse(TOKEN_URL)
                    .map_err(|error| ProviderError::InvalidEndpoint(error.to_string()))?,
                headers: json_headers(),
                body: serde_json::to_vec(&body).map_err(|_| {
                    ProviderError::InvalidResponse("OAuth request serialization failed".into())
                })?,
            })
        }
        OAuthGrant::RefreshToken => {
            let body = serde_json::json!({
                "grant_type": "refresh_token",
                "refresh_token": code,
                "client_id": CLIENT_ID,
            });
            Ok(OAuthRequestPlan {
                method: Method::POST,
                url: Url::parse(TOKEN_URL)
                    .map_err(|error| ProviderError::InvalidEndpoint(error.to_string()))?,
                headers: json_headers(),
                body: serde_json::to_vec(&body).map_err(|_| {
                    ProviderError::InvalidResponse("OAuth request serialization failed".into())
                })?,
            })
        }
    }
}

pub(crate) fn parse_token(body: &[u8]) -> Result<OAuthTokenMaterial, ProviderError> {
    let response = serde_json::from_slice::<ClaudeOAuthResponse>(body)
        .map_err(|_| ProviderError::InvalidResponse("Claude OAuth response is invalid".into()))?;
    OAuthTokenMaterial::new(
        ProviderKind::Claude,
        response.access_token,
        response.refresh_token,
        None,
        response
            .expires_in
            .map(|seconds| unix_now().saturating_add(seconds)),
        None,
        response
            .account
            .as_ref()
            .and_then(|account| account.email_address.clone())
            .or(response.email),
    )
}

pub(crate) fn routing_profile() -> Result<OAuthRoutingProfile, ProviderError> {
    OAuthRoutingProfile::fixed(DATA_BASE_URL, ProtocolDialect::AnthropicMessages, MODELS)
}

pub(crate) fn credential_headers(
    token: &OAuthTokenMaterial,
    forwarded: &HeaderMap,
) -> Result<crate::api::CredentialHeaders, ProviderError> {
    if token.provider() != ProviderKind::Claude {
        return Err(ProviderError::InvalidCredential(
            "OAuth token provider does not match Claude".into(),
        ));
    }
    let authorization = HeaderValue::from_str(&format!("Bearer {}", token.access_token()))
        .map_err(|_| {
            ProviderError::InvalidCredential("invalid OAuth access token header".into())
        })?;
    let mut headers = HeaderMap::new();
    headers.insert(header::AUTHORIZATION, authorization);
    headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
    headers.insert(
        "anthropic-beta",
        merged_oauth_betas(forwarded.get("anthropic-beta"))?,
    );
    Ok(crate::api::CredentialHeaders { headers })
}

fn merged_oauth_betas(existing: Option<&HeaderValue>) -> Result<HeaderValue, ProviderError> {
    const REQUIRED: &str = "oauth-2025-04-20";
    let existing = existing
        .map(HeaderValue::to_str)
        .transpose()
        .map_err(|_| ProviderError::InvalidCredential("invalid anthropic-beta header".into()))?
        .unwrap_or_default();
    let contains_required = existing
        .split(',')
        .any(|value| value.trim().eq_ignore_ascii_case(REQUIRED));
    let merged = if existing.trim().is_empty() {
        REQUIRED.to_owned()
    } else if contains_required {
        existing.to_owned()
    } else {
        format!("{existing},{REQUIRED}")
    };
    HeaderValue::from_str(&merged)
        .map_err(|_| ProviderError::InvalidCredential("invalid anthropic-beta header".into()))
}

#[derive(Deserialize)]
struct ClaudeOAuthResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
    account: Option<ClaudeAccount>,
    email: Option<String>,
}

#[derive(Deserialize)]
struct ClaudeAccount {
    email_address: Option<String>,
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or(i64::MAX)
}
