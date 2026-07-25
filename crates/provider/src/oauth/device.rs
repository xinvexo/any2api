use std::fmt;

use secrecy::{ExposeSecret, SecretString};
use url::Url;

use crate::{OAuthTokenMaterial, ProviderError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OAuthLoginFlow {
    AuthorizationCodePkce,
    DeviceCode,
}

pub struct OAuthDeviceAuthorization {
    device_code: SecretString,
    user_code: String,
    verification_uri: Url,
    verification_uri_complete: Option<Url>,
    expires_in_seconds: u64,
    poll_interval_seconds: u64,
}

impl OAuthDeviceAuthorization {
    pub fn new(
        device_code: String,
        user_code: String,
        verification_uri: Url,
        verification_uri_complete: Option<Url>,
        expires_in_seconds: u64,
        poll_interval_seconds: u64,
    ) -> Result<Self, ProviderError> {
        if device_code.trim().is_empty()
            || user_code.trim().is_empty()
            || expires_in_seconds == 0
            || poll_interval_seconds == 0
        {
            return Err(ProviderError::InvalidResponse(
                "OAuth device authorization response is invalid".into(),
            ));
        }
        Ok(Self {
            device_code: SecretString::from(device_code),
            user_code,
            verification_uri,
            verification_uri_complete,
            expires_in_seconds,
            poll_interval_seconds,
        })
    }

    #[must_use]
    pub fn device_code(&self) -> &str {
        self.device_code.expose_secret()
    }

    #[must_use]
    pub fn user_code(&self) -> &str {
        &self.user_code
    }

    #[must_use]
    pub const fn verification_uri(&self) -> &Url {
        &self.verification_uri
    }

    #[must_use]
    pub const fn verification_uri_complete(&self) -> Option<&Url> {
        self.verification_uri_complete.as_ref()
    }

    #[must_use]
    pub const fn expires_in_seconds(&self) -> u64 {
        self.expires_in_seconds
    }

    #[must_use]
    pub const fn poll_interval_seconds(&self) -> u64 {
        self.poll_interval_seconds
    }
}

impl fmt::Debug for OAuthDeviceAuthorization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OAuthDeviceAuthorization")
            .field("device_code", &"[REDACTED]")
            .field("user_code_present", &!self.user_code.is_empty())
            .field("verification_uri", &self.verification_uri)
            .field(
                "verification_uri_complete_present",
                &self.verification_uri_complete.is_some(),
            )
            .field("expires_in_seconds", &self.expires_in_seconds)
            .field("poll_interval_seconds", &self.poll_interval_seconds)
            .finish()
    }
}

#[derive(Debug)]
pub enum OAuthDeviceTokenPoll {
    Pending,
    SlowDown,
    Authorized(OAuthTokenMaterial),
    Denied,
    Expired,
}
