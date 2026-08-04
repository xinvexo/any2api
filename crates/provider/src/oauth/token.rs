use std::fmt;

use any2api_domain::ProviderKind;
use http::{HeaderMap, HeaderValue, Method, header};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::ProviderError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OAuthGrant {
    AuthorizationCode,
    RefreshToken,
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
        if !provider.supports_oauth() {
            return Err(ProviderError::InvalidResponse(
                "OAuth2 is not supported by this provider".into(),
            ));
        }
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

    pub fn merge_refresh_response(mut self, previous: &Self) -> Result<Self, ProviderError> {
        if self.provider != previous.provider {
            return Err(ProviderError::InvalidResponse(
                "OAuth refresh response provider does not match the account".into(),
            ));
        }
        if self.refresh_token.is_none() {
            self.refresh_token = previous.refresh_token.clone();
        }
        if self.id_token.is_none() {
            self.id_token = previous.id_token.clone();
        }
        if self.expires_at.is_none() {
            self.expires_at = previous.expires_at;
        }
        if self.account_id.is_none() {
            self.account_id.clone_from(&previous.account_id);
        }
        if self.email.is_none() {
            self.email.clone_from(&previous.email);
        }
        Ok(self)
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

pub(crate) fn form_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/x-www-form-urlencoded"),
    );
    headers
}

pub(crate) fn json_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers
}

pub fn encode_oauth_account_document(token: &OAuthTokenMaterial) -> Result<Vec<u8>, ProviderError> {
    serde_json::to_vec_pretty(&OAuthAccountDocumentRef {
        access_token: token.access_token(),
        refresh_token: token.refresh_token(),
        id_token: token.id_token(),
        account_id: token.account_id(),
        email: token.email(),
    })
    .map(|mut bytes| {
        bytes.push(b'\n');
        bytes
    })
    .map_err(|_| ProviderError::InvalidResponse("OAuth document serialization failed".into()))
}

pub fn decode_oauth_account_document(
    provider: ProviderKind,
    expires_at: Option<i64>,
    bytes: &[u8],
) -> Result<OAuthTokenMaterial, ProviderError> {
    let document = serde_json::from_slice::<OAuthAccountDocumentFields>(bytes)
        .map_err(|_| invalid_document())?;
    OAuthTokenMaterial::new(
        provider,
        document.access_token,
        document.refresh_token,
        document.id_token,
        expires_at,
        document.account_id,
        document.email,
    )
    .map_err(|_| invalid_document())
}

#[derive(Serialize)]
struct OAuthAccountDocumentRef<'a> {
    access_token: &'a str,
    refresh_token: Option<&'a str>,
    id_token: Option<&'a str>,
    account_id: Option<&'a str>,
    email: Option<&'a str>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OAuthAccountDocumentFields {
    access_token: String,
    refresh_token: Option<String>,
    id_token: Option<String>,
    account_id: Option<String>,
    email: Option<String>,
}

fn invalid_document() -> ProviderError {
    ProviderError::InvalidResponse("OAuth account document is invalid".into())
}

fn optional_secret(value: Option<String>) -> Option<SecretString> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(SecretString::from)
}

fn optional_text(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}
