//! OpenAI/Codex OAuth contract.

use any2api_domain::{ProtocolDialect, ProviderKind};
use http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use serde::Deserialize;
use url::Url;

use crate::{
    ProviderError,
    api::{
        OAuthGrant, OAuthPrincipalIdentity, OAuthRefreshRejection, OAuthRequestPlan,
        OAuthRoutingProfile, OAuthTokenMaterial,
    },
    oauth::{
        email_principal_identity, expires_at_from_duration, form_headers, json_headers,
        workspace_member_principal_identity,
    },
};

use super::claims::decode as decode_claims;

const AUTHORIZE_URL: &str = "https://auth.openai.com/oauth/authorize";
const TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const REDIRECT_URI: &str = "http://localhost:1455/auth/callback";
const DATA_BASE_URL: &str = "https://chatgpt.com/backend-api/codex";

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
            let body = serde_json::json!({
                "client_id": CLIENT_ID,
                "grant_type": "refresh_token",
                "refresh_token": code,
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

pub(crate) fn classify_refresh_rejection(
    status: StatusCode,
    bounded_body: &[u8],
) -> OAuthRefreshRejection {
    OAuthRefreshRejection::classify_with_codes(
        status,
        bounded_body,
        &[
            ("invalid_grant", OAuthRefreshRejection::InvalidGrant),
            (
                "refresh_token_expired",
                OAuthRefreshRejection::RefreshTokenExpired,
            ),
            (
                "refresh_token_reused",
                OAuthRefreshRejection::RefreshTokenReused,
            ),
            (
                "refresh_token_invalidated",
                OAuthRefreshRejection::RefreshTokenInvalidated,
            ),
        ],
    )
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
        Some(expires_at_from_duration(response.expires_in)?),
        claims.account_id.or(response.account_id),
        claims.email.or(response.email),
    )
}

pub(crate) fn routing_profile(
    _token: &OAuthTokenMaterial,
) -> Result<OAuthRoutingProfile, ProviderError> {
    OAuthRoutingProfile::fixed(DATA_BASE_URL, ProtocolDialect::OpenAiResponses)
}

pub(crate) fn principal_identity(token: &OAuthTokenMaterial) -> Option<OAuthPrincipalIdentity> {
    if token.provider() != ProviderKind::Codex {
        return None;
    }
    let id_claims = token
        .id_token()
        .map(|token| decode_claims(Some(token)))
        .unwrap_or_default();
    let access_claims = decode_claims(Some(token.access_token()));
    let member_id = id_claims
        .member_id
        .clone()
        .or(access_claims.member_id.clone());
    let workspace_id = token
        .account_id()
        .or(id_claims.account_id.as_deref())
        .or(access_claims.account_id.as_deref());
    member_id
        .as_deref()
        .and_then(|member_id| {
            workspace_member_principal_identity(ProviderKind::Codex, workspace_id, member_id)
        })
        .or_else(|| {
            email_principal_identity(
                ProviderKind::Codex,
                token
                    .email()
                    .or(id_claims.email.as_deref())
                    .or(access_claims.email.as_deref()),
            )
        })
}

/// Official `chatgpt_plan_type` from Codex ID Token claims (no local renaming).
#[must_use]
pub fn plan_label(token: &OAuthTokenMaterial) -> Option<String> {
    if token.provider() != ProviderKind::Codex {
        return None;
    }
    decode_claims(token.id_token())
        .plan
        .map(|plan| plan.trim().to_owned())
        .filter(|plan| !plan.is_empty())
}

pub(crate) fn credential_headers(
    token: &OAuthTokenMaterial,
) -> Result<crate::api::CredentialHeaders, ProviderError> {
    if token.provider() != ProviderKind::Codex {
        return Err(ProviderError::InvalidCredential(
            "OAuth token provider does not match Codex".into(),
        ));
    }
    let mut authorization = HeaderValue::from_str(&format!("Bearer {}", token.access_token()))
        .map_err(|_| {
            ProviderError::InvalidCredential("invalid OAuth access token header".into())
        })?;
    authorization.set_sensitive(true);
    let mut headers = HeaderMap::new();
    headers.insert(header::AUTHORIZATION, authorization);
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
    expires_in: i64,
    account_id: Option<String>,
    email: Option<String>,
}
