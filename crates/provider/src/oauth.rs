use std::fmt;

use any2api_domain::{CredentialKind, OAUTH2_SECRET_SCHEMA_VERSION, ProviderKind};
use http::{HeaderMap, HeaderValue, Method, header};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{ProviderError, ProviderSecret};

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
    organization_id: Option<String>,
    client_id: Option<String>,
}

impl OAuthTokenMaterial {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        provider: ProviderKind,
        access_token: String,
        refresh_token: Option<String>,
        id_token: Option<String>,
        expires_at: Option<i64>,
        account_id: Option<String>,
        email: Option<String>,
        organization_id: Option<String>,
        client_id: Option<String>,
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
            organization_id: optional_text(organization_id),
            client_id: optional_text(client_id),
        })
    }

    pub fn from_secret(
        provider: ProviderKind,
        secret: &ProviderSecret,
    ) -> Result<Self, ProviderError> {
        if secret.schema_version() != OAUTH2_SECRET_SCHEMA_VERSION {
            return Err(ProviderError::InvalidCredential(
                "unsupported OAuth2 secret schema".into(),
            ));
        }
        let stored = serde_json::from_str::<StoredOAuthToken>(secret.expose())
            .map_err(|_| ProviderError::InvalidCredential("OAuth2 secret is invalid".into()))?;
        if stored.provider != provider {
            return Err(ProviderError::InvalidCredential(
                "OAuth2 secret provider does not match endpoint".into(),
            ));
        }
        Self::new(
            stored.provider,
            stored.access_token,
            stored.refresh_token,
            stored.id_token,
            stored.expires_at,
            stored.account_id,
            stored.email,
            stored.organization_id,
            stored.client_id,
        )
    }

    pub fn to_secret(&self) -> Result<ProviderSecret, ProviderError> {
        let stored = StoredOAuthToken {
            provider: self.provider,
            access_token: self.access_token.expose_secret().to_owned(),
            refresh_token: self
                .refresh_token
                .as_ref()
                .map(|value| value.expose_secret().to_owned()),
            id_token: self
                .id_token
                .as_ref()
                .map(|value| value.expose_secret().to_owned()),
            expires_at: self.expires_at,
            account_id: self.account_id.clone(),
            email: self.email.clone(),
            organization_id: self.organization_id.clone(),
            client_id: self.client_id.clone(),
        };
        serde_json::to_string(&stored)
            .map(|value| ProviderSecret::new(OAUTH2_SECRET_SCHEMA_VERSION, value))
            .map_err(|_| {
                ProviderError::InvalidResponse("OAuth2 secret serialization failed".into())
            })
    }

    #[must_use]
    pub fn provider(&self) -> ProviderKind {
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
    pub fn expires_at(&self) -> Option<i64> {
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

    #[must_use]
    pub fn organization_id(&self) -> Option<&str> {
        self.organization_id.as_deref()
    }

    #[must_use]
    pub fn client_id(&self) -> Option<&str> {
        self.client_id.as_deref()
    }

    #[must_use]
    pub fn is_expired_or_near_expiry(&self, now: i64, lead_seconds: i64) -> bool {
        self.expires_at
            .is_some_and(|expires_at| expires_at <= now.saturating_add(lead_seconds))
    }

    #[must_use]
    pub fn with_fallback_from(mut self, previous: &Self) -> Self {
        if self.provider != previous.provider {
            return self;
        }
        if self.refresh_token.is_none() {
            self.refresh_token.clone_from(&previous.refresh_token);
        }
        if self.id_token.is_none() {
            self.id_token.clone_from(&previous.id_token);
        }
        if self.account_id.is_none() {
            self.account_id.clone_from(&previous.account_id);
        }
        if self.email.is_none() {
            self.email.clone_from(&previous.email);
        }
        if self.organization_id.is_none() {
            self.organization_id.clone_from(&previous.organization_id);
        }
        if self.client_id.is_none() {
            self.client_id.clone_from(&previous.client_id);
        }
        self
    }
}

impl fmt::Debug for OAuthTokenMaterial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OAuthTokenMaterial")
            .field("provider", &self.provider)
            .field("access_token", &"[REDACTED]")
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field("id_token", &self.id_token.as_ref().map(|_| "[REDACTED]"))
            .field("expires_at", &self.expires_at)
            .field("account_id", &self.account_id)
            .field("email", &self.email)
            .field("organization_id", &self.organization_id)
            .field("client_id", &self.client_id)
            .finish()
    }
}

#[derive(Deserialize, Serialize)]
struct StoredOAuthToken {
    provider: ProviderKind,
    access_token: String,
    refresh_token: Option<String>,
    id_token: Option<String>,
    expires_at: Option<i64>,
    account_id: Option<String>,
    email: Option<String>,
    organization_id: Option<String>,
    client_id: Option<String>,
}

pub fn authorization_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::ACCEPT, HeaderValue::from_static("application/json"));
    headers
}

pub fn form_headers() -> HeaderMap {
    let mut headers = authorization_headers();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/x-www-form-urlencoded"),
    );
    headers
}

pub fn json_headers() -> HeaderMap {
    let mut headers = authorization_headers();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers
}

pub fn validate_oauth_kind(kind: CredentialKind) -> Result<(), ProviderError> {
    (kind == CredentialKind::OAuth2)
        .then_some(())
        .ok_or_else(|| ProviderError::InvalidCredential("credential is not OAuth2".into()))
}

fn optional_secret(value: Option<String>) -> Option<SecretString> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(SecretString::from)
}

fn optional_text(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use any2api_domain::ProviderKind;

    use super::OAuthTokenMaterial;

    #[test]
    fn typed_secret_round_trip_and_debug_never_expose_tokens() {
        let token = OAuthTokenMaterial::new(
            ProviderKind::Codex,
            "access-secret".to_owned(),
            Some("refresh-secret".to_owned()),
            Some("id-secret".to_owned()),
            Some(1234),
            Some("account".to_owned()),
            Some("owner@example.com".to_owned()),
            None,
            Some("client".to_owned()),
        )
        .expect("token");
        let secret = token.to_secret().expect("typed secret");
        let restored =
            OAuthTokenMaterial::from_secret(ProviderKind::Codex, &secret).expect("restored token");

        assert_eq!(restored.access_token(), "access-secret");
        assert_eq!(restored.refresh_token(), Some("refresh-secret"));
        assert_eq!(restored.account_id(), Some("account"));
        let debug = format!("{restored:?}");
        assert!(!debug.contains("access-secret"));
        assert!(!debug.contains("refresh-secret"));
        assert!(!debug.contains("id-secret"));
    }

    #[test]
    fn refresh_response_inherits_rotating_fields_that_are_omitted() {
        let previous = OAuthTokenMaterial::new(
            ProviderKind::Claude,
            "old-access".to_owned(),
            Some("old-refresh".to_owned()),
            None,
            Some(100),
            Some("account".to_owned()),
            Some("owner@example.com".to_owned()),
            Some("organization".to_owned()),
            Some("client".to_owned()),
        )
        .expect("previous token");
        let refreshed = OAuthTokenMaterial::new(
            ProviderKind::Claude,
            "new-access".to_owned(),
            None,
            None,
            Some(200),
            None,
            None,
            None,
            None,
        )
        .expect("refreshed token")
        .with_fallback_from(&previous);

        assert_eq!(refreshed.access_token(), "new-access");
        assert_eq!(refreshed.refresh_token(), Some("old-refresh"));
        assert_eq!(refreshed.email(), Some("owner@example.com"));
        assert_eq!(refreshed.organization_id(), Some("organization"));
    }
}
