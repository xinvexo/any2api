//! xAI OAuth and Grok Build subscription-data-plane contract.

use any2api_domain::{ProtocolDialect, ProviderKind};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use http::{HeaderMap, HeaderValue, Method, header};
use serde::Deserialize;
use url::Url;

use crate::{
    ProviderError,
    api::{
        CredentialHeaders, OAuthGrant, OAuthRequestPlan, OAuthRoutingProfile, OAuthTokenMaterial,
    },
    oauth::form_headers,
};

const AUTHORIZE_URL: &str = "https://auth.x.ai/oauth2/authorize";
const TOKEN_URL: &str = "https://auth.x.ai/oauth2/token";
const CLIENT_ID: &str = "b1a00492-073a-47ea-816f-4c329264a828";
const REDIRECT_URI: &str = "http://127.0.0.1:56121/callback";
const SCOPE: &str = "openid profile email offline_access grok-cli:access api:access";
const DATA_BASE_URL: &str = "https://cli-chat-proxy.grok.com/v1";
const CLI_VERSION: &str = "0.2.93";
const MODELS: &[&str] = &[
    "grok-4.5",
    "grok-4.3",
    "grok-build-0.1",
    "grok-composer-2.5-fast",
    "grok-4.20-0309-reasoning",
    "grok-4.20-0309-non-reasoning",
    "grok-4.20-multi-agent-0309",
];

pub(crate) const fn redirect_uri() -> &'static str {
    REDIRECT_URI
}

pub(crate) fn authorization_url(state: &str, code_challenge: &str) -> Result<Url, ProviderError> {
    let mut url = Url::parse(AUTHORIZE_URL)
        .map_err(|error| ProviderError::InvalidEndpoint(error.to_string()))?;
    url.query_pairs_mut()
        .append_pair("client_id", CLIENT_ID)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", REDIRECT_URI)
        .append_pair("scope", SCOPE)
        .append_pair("state", state)
        .append_pair("nonce", state)
        .append_pair("code_challenge", code_challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("plan", "generic")
        .append_pair("referrer", "any2api");
    Ok(url)
}

pub(crate) fn token_request(
    grant: OAuthGrant,
    code: &str,
    code_verifier: Option<&str>,
) -> Result<OAuthRequestPlan, ProviderError> {
    let mut form = url::form_urlencoded::Serializer::new(String::new());
    form.append_pair("client_id", CLIENT_ID);
    match grant {
        OAuthGrant::AuthorizationCode => {
            form.append_pair("grant_type", "authorization_code")
                .append_pair("code", code)
                .append_pair("redirect_uri", REDIRECT_URI)
                .append_pair(
                    "code_verifier",
                    code_verifier.ok_or_else(|| {
                        ProviderError::InvalidCredential("OAuth code verifier is required".into())
                    })?,
                );
        }
        OAuthGrant::RefreshToken => {
            form.append_pair("grant_type", "refresh_token")
                .append_pair("refresh_token", code);
        }
    }
    Ok(OAuthRequestPlan {
        method: Method::POST,
        url: Url::parse(TOKEN_URL)
            .map_err(|error| ProviderError::InvalidEndpoint(error.to_string()))?,
        headers: form_headers(),
        body: form.finish().into_bytes(),
    })
}

pub(crate) fn parse_token(body: &[u8]) -> Result<OAuthTokenMaterial, ProviderError> {
    let response = serde_json::from_slice::<GrokOAuthResponse>(body)
        .map_err(|_| ProviderError::InvalidResponse("Grok OAuth response is invalid".into()))?;
    let claims = decode_claims(response.id_token.as_deref())
        .or_else(|| decode_claims(Some(&response.access_token)))
        .unwrap_or_default();
    OAuthTokenMaterial::new(
        ProviderKind::Grok,
        response.access_token,
        response.refresh_token,
        response.id_token,
        response
            .expires_in
            .map(|seconds| unix_now().saturating_add(seconds)),
        claims.subject.or(response.subject),
        claims.email.or(response.email),
    )
}

pub(crate) fn routing_profile() -> Result<OAuthRoutingProfile, ProviderError> {
    OAuthRoutingProfile::fixed(DATA_BASE_URL, ProtocolDialect::OpenAiResponses, MODELS)
}

pub(crate) fn credential_headers(
    token: &OAuthTokenMaterial,
) -> Result<CredentialHeaders, ProviderError> {
    if token.provider() != ProviderKind::Grok {
        return Err(ProviderError::InvalidCredential(
            "OAuth token provider does not match Grok".into(),
        ));
    }
    let authorization = HeaderValue::from_str(&format!("Bearer {}", token.access_token()))
        .map_err(|_| {
            ProviderError::InvalidCredential("invalid OAuth access token header".into())
        })?;
    let mut headers = HeaderMap::new();
    headers.insert(header::AUTHORIZATION, authorization);
    headers.insert("x-xai-token-auth", HeaderValue::from_static("xai-grok-cli"));
    headers.insert(
        "x-grok-client-version",
        HeaderValue::from_static(CLI_VERSION),
    );
    headers.insert(
        header::USER_AGENT,
        HeaderValue::from_static("xai-grok-workspace/0.2.93"),
    );
    Ok(CredentialHeaders { headers })
}

#[derive(Deserialize)]
struct GrokOAuthResponse {
    access_token: String,
    refresh_token: Option<String>,
    id_token: Option<String>,
    expires_in: Option<i64>,
    email: Option<String>,
    #[serde(rename = "sub")]
    subject: Option<String>,
}

#[derive(Default, Deserialize)]
struct Claims {
    email: Option<String>,
    #[serde(rename = "sub")]
    subject: Option<String>,
}

fn decode_claims(token: Option<&str>) -> Option<Claims> {
    let payload = token?.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or(i64::MAX)
}
