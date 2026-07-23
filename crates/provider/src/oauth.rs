use std::fmt;

use any2api_domain::ProviderKind;
use http::{HeaderMap, HeaderValue, Method, header};
use secrecy::{ExposeSecret, SecretString};
use serde::Serialize;
use url::Url;

use crate::ProviderError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OAuthGrant {
    AuthorizationCode,
}

#[derive(Clone)]
pub struct OAuthRequestPlan {
    pub method: Method,
    pub url: Url,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl fmt::Debug for OAuthRequestPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OAuthRequestPlan")
            .field("method", &self.method)
            .field("url", &self.url)
            .field("header_count", &self.headers.len())
            .field("body_bytes", &self.body.len())
            .finish()
    }
}

#[derive(Clone)]
pub struct OAuthTokenMaterial {
    provider: ProviderKind,
    access_token: SecretString,
    refresh_token: Option<SecretString>,
    id_token: Option<SecretString>,
    expires_at: Option<i64>,
    account_id: Option<String>,
    email: Option<String>,
}

impl OAuthTokenMaterial {
    pub fn new(
        provider: ProviderKind,
        access_token: String,
        refresh_token: Option<String>,
        id_token: Option<String>,
        expires_at: Option<i64>,
        account_id: Option<String>,
        email: Option<String>,
    ) -> Result<Self, ProviderError> {
        if access_token.trim().is_empty() {
            return Err(ProviderError::InvalidResponse(
                "OAuth response did not contain an access token".into(),
            ));
        }
        Ok(Self {
            provider,
            access_token: SecretString::from(access_token),
            refresh_token: optional_secret(refresh_token),
            id_token: optional_secret(id_token),
            expires_at,
            account_id: optional_text(account_id),
            email: optional_text(email),
        })
    }

    #[must_use]
    pub const fn provider(&self) -> ProviderKind {
        self.provider
    }

    #[must_use]
    pub fn access_token(&self) -> &str {
        self.access_token.expose_secret()
    }

    #[must_use]
    pub fn refresh_token(&self) -> Option<&str> {
        self.refresh_token.as_ref().map(ExposeSecret::expose_secret)
    }

    #[must_use]
    pub fn id_token(&self) -> Option<&str> {
        self.id_token.as_ref().map(ExposeSecret::expose_secret)
    }

    #[must_use]
    pub const fn expires_at(&self) -> Option<i64> {
        self.expires_at
    }

    #[must_use]
    pub fn account_id(&self) -> Option<&str> {
        self.account_id.as_deref()
    }

    #[must_use]
    pub fn email(&self) -> Option<&str> {
        self.email.as_deref()
    }
}

impl fmt::Debug for OAuthTokenMaterial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OAuthTokenMaterial")
            .field("provider", &self.provider)
            .field("access_token", &"[REDACTED]")
            .field("refresh_token_present", &self.refresh_token.is_some())
            .field("id_token_present", &self.id_token.is_some())
            .field("expires_at", &self.expires_at)
            .field("account_id_present", &self.account_id.is_some())
            .field("email_present", &self.email.is_some())
            .finish()
    }
}

pub fn form_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/x-www-form-urlencoded"),
    );
    headers
}

pub fn json_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers
}

pub fn serialize_file(
    token: &OAuthTokenMaterial,
    last_refresh: &str,
    expired: &str,
) -> Result<Vec<u8>, ProviderError> {
    let empty = "";
    let encoded = match token.provider() {
        ProviderKind::Codex => serde_json::to_vec_pretty(&CodexOAuthFile {
            id_token: token.id_token().unwrap_or(empty),
            access_token: token.access_token(),
            refresh_token: token.refresh_token().unwrap_or(empty),
            account_id: token.account_id().unwrap_or(empty),
            last_refresh,
            email: token.email().unwrap_or(empty),
            provider_type: "codex",
            expired,
        }),
        ProviderKind::Claude => serde_json::to_vec_pretty(&ClaudeOAuthFile {
            id_token: token.id_token().unwrap_or(empty),
            access_token: token.access_token(),
            refresh_token: token.refresh_token().unwrap_or(empty),
            last_refresh,
            email: token.email().unwrap_or(empty),
            provider_type: "claude",
            expired,
        }),
    };
    encoded
        .map(|mut bytes| {
            bytes.push(b'\n');
            bytes
        })
        .map_err(|_| ProviderError::InvalidResponse("OAuth file serialization failed".into()))
}

#[derive(Serialize)]
struct CodexOAuthFile<'a> {
    id_token: &'a str,
    access_token: &'a str,
    refresh_token: &'a str,
    account_id: &'a str,
    last_refresh: &'a str,
    email: &'a str,
    #[serde(rename = "type")]
    provider_type: &'a str,
    expired: &'a str,
}

#[derive(Serialize)]
struct ClaudeOAuthFile<'a> {
    id_token: &'a str,
    access_token: &'a str,
    refresh_token: &'a str,
    last_refresh: &'a str,
    email: &'a str,
    #[serde(rename = "type")]
    provider_type: &'a str,
    expired: &'a str,
}

fn optional_secret(value: Option<String>) -> Option<SecretString> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(SecretString::from)
}

fn optional_text(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}
