//! xAI OAuth and Grok Build subscription-data-plane contract.

use any2api_domain::{ProtocolDialect, ProviderKind};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use serde::Deserialize;
use url::Url;

use crate::{
    ProviderError,
    api::{
        CredentialHeaders, OAuthDeviceAuthorization, OAuthDeviceTokenPoll, OAuthRequestPlan,
        OAuthRoutingProfile, OAuthTokenMaterial,
    },
    oauth::{expires_at_from_duration, form_headers},
};

const DEVICE_AUTHORIZATION_URL: &str = "https://auth.x.ai/oauth2/device/code";
const TOKEN_URL: &str = "https://auth.x.ai/oauth2/token";
const CLIENT_ID: &str = "b1a00492-073a-47ea-816f-4c329264a828";
const SCOPE: &str = "openid profile email offline_access grok-cli:access api:access";
const DEVICE_CODE_GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:device_code";
const DEFAULT_POLL_INTERVAL_SECONDS: u64 = 5;
const DATA_BASE_URL: &str = "https://cli-chat-proxy.grok.com/v1";

pub(crate) fn device_authorization_request() -> Result<OAuthRequestPlan, ProviderError> {
    let mut form = url::form_urlencoded::Serializer::new(String::new());
    form.append_pair("client_id", CLIENT_ID)
        .append_pair("scope", SCOPE);
    Ok(OAuthRequestPlan {
        method: Method::POST,
        url: Url::parse(DEVICE_AUTHORIZATION_URL)
            .map_err(|error| ProviderError::InvalidEndpoint(error.to_string()))?,
        headers: form_headers(),
        body: form.finish().into_bytes(),
    })
}

pub(crate) fn parse_device_authorization(
    body: &[u8],
) -> Result<OAuthDeviceAuthorization, ProviderError> {
    let response =
        serde_json::from_slice::<GrokDeviceAuthorizationResponse>(body).map_err(|_| {
            ProviderError::InvalidResponse("Grok device authorization response is invalid".into())
        })?;
    let complete = response
        .verification_uri_complete
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(parse_verification_uri)
        .transpose()?;
    let verification_uri = if response.verification_uri.trim().is_empty() {
        complete.clone().ok_or_else(|| {
            ProviderError::InvalidResponse(
                "Grok device authorization response is missing a verification URI".into(),
            )
        })?
    } else {
        parse_verification_uri(&response.verification_uri)?
    };
    OAuthDeviceAuthorization::new(
        response.device_code,
        response.user_code,
        verification_uri,
        complete,
        response.expires_in,
        response
            .interval
            .unwrap_or(DEFAULT_POLL_INTERVAL_SECONDS)
            .max(DEFAULT_POLL_INTERVAL_SECONDS),
    )
}

pub(crate) fn device_token_request(device_code: &str) -> Result<OAuthRequestPlan, ProviderError> {
    if device_code.trim().is_empty() {
        return Err(ProviderError::InvalidCredential(
            "OAuth device code is required".into(),
        ));
    }
    let mut form = url::form_urlencoded::Serializer::new(String::new());
    form.append_pair("grant_type", DEVICE_CODE_GRANT_TYPE)
        .append_pair("client_id", CLIENT_ID)
        .append_pair("device_code", device_code);
    token_plan(form.finish())
}

pub(crate) fn parse_device_token(
    status: StatusCode,
    body: &[u8],
) -> Result<OAuthDeviceTokenPoll, ProviderError> {
    let value = serde_json::from_slice::<serde_json::Value>(body).map_err(|_| {
        ProviderError::InvalidResponse("Grok device token response is invalid".into())
    })?;
    if let Some(error) = value.get("error").and_then(serde_json::Value::as_str) {
        return match error {
            "authorization_pending" => Ok(OAuthDeviceTokenPoll::Pending),
            "slow_down" => Ok(OAuthDeviceTokenPoll::SlowDown),
            "access_denied" => Ok(OAuthDeviceTokenPoll::Denied),
            "expired_token" => Ok(OAuthDeviceTokenPoll::Expired),
            _ => Err(ProviderError::InvalidResponse(
                "Grok device token request was rejected".into(),
            )),
        };
    }
    if !status.is_success() {
        return Err(ProviderError::InvalidResponse(
            "Grok device token request was rejected".into(),
        ));
    }
    parse_token(body).map(OAuthDeviceTokenPoll::Authorized)
}

pub(crate) fn refresh_request(refresh_token: &str) -> Result<OAuthRequestPlan, ProviderError> {
    let mut form = url::form_urlencoded::Serializer::new(String::new());
    form.append_pair("client_id", CLIENT_ID)
        .append_pair("grant_type", "refresh_token")
        .append_pair("refresh_token", refresh_token);
    token_plan(form.finish())
}

fn token_plan(body: String) -> Result<OAuthRequestPlan, ProviderError> {
    Ok(OAuthRequestPlan {
        method: Method::POST,
        url: Url::parse(TOKEN_URL)
            .map_err(|error| ProviderError::InvalidEndpoint(error.to_string()))?,
        headers: form_headers(),
        body: body.into_bytes(),
    })
}

pub(crate) fn parse_token(body: &[u8]) -> Result<OAuthTokenMaterial, ProviderError> {
    let response = serde_json::from_slice::<GrokOAuthResponse>(body)
        .map_err(|_| ProviderError::InvalidResponse("Grok OAuth response is invalid".into()))?;
    let claims = identity_claims(response.id_token.as_deref(), &response.access_token);
    OAuthTokenMaterial::new(
        ProviderKind::Grok,
        response.access_token,
        response.refresh_token,
        response.id_token,
        Some(expires_at_from_duration(response.expires_in)?),
        claims.subject.or(response.subject),
        claims.email.or(response.email),
    )
}

pub(crate) fn routing_profile() -> Result<OAuthRoutingProfile, ProviderError> {
    OAuthRoutingProfile::fixed(DATA_BASE_URL, ProtocolDialect::OpenAiResponses)
}

pub(crate) fn credential_headers(
    token: &OAuthTokenMaterial,
) -> Result<CredentialHeaders, ProviderError> {
    if token.provider() != ProviderKind::Grok {
        return Err(ProviderError::InvalidCredential(
            "OAuth token provider does not match Grok".into(),
        ));
    }
    let mut authorization = HeaderValue::from_str(&format!("Bearer {}", token.access_token()))
        .map_err(|_| {
            ProviderError::InvalidCredential("invalid OAuth access token header".into())
        })?;
    authorization.set_sensitive(true);
    let mut headers = HeaderMap::new();
    headers.insert(header::AUTHORIZATION, authorization);
    if let Some(subject) = token.account_id() {
        let subject = HeaderValue::from_str(subject).map_err(|_| {
            ProviderError::InvalidCredential("invalid Grok OAuth subject header".into())
        })?;
        headers.insert("x-userid", subject);
    }
    Ok(CredentialHeaders { headers })
}

pub(crate) fn bot_flagged(token: &OAuthTokenMaterial) -> Option<bool> {
    if token.provider() != ProviderKind::Grok {
        return None;
    }
    let claims = decode_json_claims(token.access_token())?;
    match claims.get("bot_flag_source")?.as_i64()? {
        1 => Some(true),
        0 => Some(false),
        _ => None,
    }
}

#[derive(Deserialize)]
struct GrokOAuthResponse {
    access_token: String,
    refresh_token: Option<String>,
    id_token: Option<String>,
    expires_in: i64,
    email: Option<String>,
    #[serde(rename = "sub")]
    subject: Option<String>,
}

#[derive(Deserialize)]
struct GrokDeviceAuthorizationResponse {
    device_code: String,
    user_code: String,
    #[serde(default)]
    verification_uri: String,
    verification_uri_complete: Option<String>,
    expires_in: u64,
    interval: Option<u64>,
}

#[derive(Default, Deserialize)]
struct Claims {
    email: Option<String>,
    #[serde(rename = "sub")]
    subject: Option<String>,
}

fn decode_claims(token: Option<&str>) -> Option<Claims> {
    serde_json::from_value(decode_json_claims(token?)?).ok()
}

fn identity_claims(id_token: Option<&str>, access_token: &str) -> Claims {
    let mut claims = decode_claims(id_token).unwrap_or_default();
    let fallback = decode_claims(Some(access_token)).unwrap_or_default();
    claims.subject = claims.subject.or(fallback.subject);
    claims.email = claims.email.or(fallback.email);
    claims
}

fn decode_json_claims(token: &str) -> Option<serde_json::Value> {
    let payload = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn parse_verification_uri(value: &str) -> Result<Url, ProviderError> {
    let url = Url::parse(value).map_err(|_| {
        ProviderError::InvalidResponse("Grok device verification URI is invalid".into())
    })?;
    let host = url.host_str().unwrap_or_default();
    if url.scheme() != "https" || (host != "x.ai" && !host.ends_with(".x.ai")) {
        return Err(ProviderError::InvalidResponse(
            "Grok device verification URI is invalid".into(),
        ));
    }
    Ok(url)
}
