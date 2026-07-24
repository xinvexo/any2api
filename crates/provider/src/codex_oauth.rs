use any2api_domain::{ProtocolDialect, ProviderKind};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use http::{HeaderMap, HeaderValue, Method, header};
use serde::Deserialize;
use url::Url;

use crate::{
    ProviderError,
    api::{OAuthGrant, OAuthRequestPlan, OAuthRoutingProfile, OAuthTokenMaterial},
    oauth::form_headers,
};

const AUTHORIZE_URL: &str = "https://auth.openai.com/oauth/authorize";
const TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const REDIRECT_URI: &str = "http://localhost:1455/auth/callback";
const DATA_BASE_URL: &str = "https://chatgpt.com/backend-api/codex";
const FREE_MODELS: &[&str] = &[
    "codex-auto-review",
    "gpt-5.4-mini",
    "gpt-5.5",
    "gpt-5.6-luna",
    "gpt-5.6-terra",
];
const TEAM_MODELS: &[&str] = &[
    "codex-auto-review",
    "gpt-5.4",
    "gpt-5.4-mini",
    "gpt-5.5",
    "gpt-5.6-luna",
    "gpt-5.6-sol",
    "gpt-5.6-terra",
];
const PLUS_MODELS: &[&str] = &[
    "codex-auto-review",
    "gpt-5.3-codex-spark",
    "gpt-5.4",
    "gpt-5.4-mini",
    "gpt-5.5",
    "gpt-5.6-luna",
    "gpt-5.6-sol",
    "gpt-5.6-terra",
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
        .append_pair("scope", "openid profile email offline_access")
        .append_pair("state", state)
        .append_pair("code_challenge", code_challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("prompt", "login")
        .append_pair("id_token_add_organizations", "true")
        .append_pair("codex_cli_simplified_flow", "true");
    Ok(url)
}

pub(crate) fn token_request(
    grant: OAuthGrant,
    code: &str,
    code_verifier: Option<&str>,
) -> Result<OAuthRequestPlan, ProviderError> {
    match grant {
        OAuthGrant::AuthorizationCode => {
            let verifier = code_verifier.ok_or_else(|| {
                ProviderError::InvalidCredential("OAuth code verifier is required".into())
            })?;
            let mut form = url::form_urlencoded::Serializer::new(String::new());
            form.append_pair("client_id", CLIENT_ID)
                .append_pair("grant_type", "authorization_code")
                .append_pair("code", code)
                .append_pair("redirect_uri", REDIRECT_URI)
                .append_pair("code_verifier", verifier);
            Ok(OAuthRequestPlan {
                method: Method::POST,
                url: Url::parse(TOKEN_URL)
                    .map_err(|error| ProviderError::InvalidEndpoint(error.to_string()))?,
                headers: form_headers(),
                body: form.finish().into_bytes(),
            })
        }
        OAuthGrant::RefreshToken => {
            let mut form = url::form_urlencoded::Serializer::new(String::new());
            form.append_pair("client_id", CLIENT_ID)
                .append_pair("grant_type", "refresh_token")
                .append_pair("refresh_token", code);
            Ok(OAuthRequestPlan {
                method: Method::POST,
                url: Url::parse(TOKEN_URL)
                    .map_err(|error| ProviderError::InvalidEndpoint(error.to_string()))?,
                headers: form_headers(),
                body: form.finish().into_bytes(),
            })
        }
    }
}

pub(crate) fn parse_token(body: &[u8]) -> Result<OAuthTokenMaterial, ProviderError> {
    let response = serde_json::from_slice::<CodexOAuthResponse>(body)
        .map_err(|_| ProviderError::InvalidResponse("Codex OAuth response is invalid".into()))?;
    let claims = decode_claims(response.id_token.as_deref());
    OAuthTokenMaterial::new(
        ProviderKind::Codex,
        response.access_token,
        response.refresh_token,
        response.id_token,
        response
            .expires_in
            .map(|seconds| unix_now().saturating_add(seconds)),
        claims.account_id.or(response.account_id),
        claims.email.or(response.email),
    )
}

pub(crate) fn routing_profile(
    token: &OAuthTokenMaterial,
) -> Result<OAuthRoutingProfile, ProviderError> {
    let claims = decode_claims(token.id_token());
    OAuthRoutingProfile::fixed(
        DATA_BASE_URL,
        ProtocolDialect::OpenAiResponses,
        models_for_plan(claims.plan.as_deref()),
    )
}

pub(crate) fn credential_headers(
    token: &OAuthTokenMaterial,
) -> Result<crate::api::CredentialHeaders, ProviderError> {
    if token.provider() != ProviderKind::Codex {
        return Err(ProviderError::InvalidCredential(
            "OAuth token provider does not match Codex".into(),
        ));
    }
    let authorization = HeaderValue::from_str(&format!("Bearer {}", token.access_token()))
        .map_err(|_| {
            ProviderError::InvalidCredential("invalid OAuth access token header".into())
        })?;
    let mut headers = HeaderMap::new();
    headers.insert(header::AUTHORIZATION, authorization);
    headers.insert("originator", HeaderValue::from_static("codex_cli_rs"));
    if let Some(account_id) = token.account_id() {
        let account_id = HeaderValue::from_str(account_id).map_err(|_| {
            ProviderError::InvalidCredential("invalid Codex OAuth account id header".into())
        })?;
        headers.insert("chatgpt-account-id", account_id);
    }
    Ok(crate::api::CredentialHeaders { headers })
}

#[derive(Deserialize)]
struct CodexOAuthResponse {
    access_token: String,
    refresh_token: Option<String>,
    id_token: Option<String>,
    expires_in: Option<i64>,
    account_id: Option<String>,
    email: Option<String>,
}

#[derive(Default)]
struct DecodedClaims {
    account_id: Option<String>,
    email: Option<String>,
    plan: Option<String>,
}

fn decode_claims(id_token: Option<&str>) -> DecodedClaims {
    let Some(payload) = id_token.and_then(|token| token.split('.').nth(1)) else {
        return DecodedClaims::default();
    };
    let Ok(bytes) = URL_SAFE_NO_PAD.decode(payload) else {
        return DecodedClaims::default();
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
        chatgpt_plan_type: Option<String>,
    }
    let Ok(claims) = serde_json::from_slice::<Claims>(&bytes) else {
        return DecodedClaims::default();
    };
    let (account_id, plan) = claims
        .auth
        .map(|auth| (auth.chatgpt_account_id, auth.chatgpt_plan_type))
        .unwrap_or_default();
    DecodedClaims {
        account_id,
        email: claims.email,
        plan,
    }
}

fn models_for_plan(plan: Option<&str>) -> &'static [&'static str] {
    let Some(plan) = plan.map(str::trim).filter(|plan| !plan.is_empty()) else {
        return FREE_MODELS;
    };
    if plan.eq_ignore_ascii_case("pro") || plan.eq_ignore_ascii_case("plus") {
        PLUS_MODELS
    } else if plan.eq_ignore_ascii_case("team")
        || plan.eq_ignore_ascii_case("business")
        || plan.eq_ignore_ascii_case("go")
    {
        TEAM_MODELS
    } else {
        FREE_MODELS
    }
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or(i64::MAX)
}
