use any2api_provider::api::{OAuthQuotaUsage, ProviderError};
use any2api_transport::api::TransportError;
use thiserror::Error;

#[derive(Clone, Debug, PartialEq)]
pub struct OAuthQuotaSnapshot {
    pub usage: OAuthQuotaUsage,
    pub fetched_at: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OAuthQuotaResetOutcome {
    pub windows_reset: u32,
}

#[derive(Debug, Error)]
pub enum OAuthQuotaError {
    #[error("OAuth account was not found")]
    AccountNotFound,
    #[error("OAuth provider driver is unavailable")]
    ProviderUnavailable,
    #[error("OAuth quota is not supported by this provider")]
    UnsupportedProvider,
    #[error("OAuth account runtime is unavailable")]
    RuntimeUnavailable,
    #[error("OAuth account is at its concurrency limit")]
    CredentialAtCapacity,
    #[error("OAuth token material is unavailable")]
    TokenMaterialUnavailable,
    #[error("OAuth proxy path is unavailable")]
    ProxyUnavailable,
    #[error("OAuth quota request URI is invalid")]
    InvalidEndpointUri,
    #[error("OAuth quota provider request is invalid")]
    Provider(#[source] ProviderError),
    #[error("OAuth quota transport failed")]
    Transport(#[source] TransportError),
    #[error("OAuth quota response read timed out")]
    ReadTimeout,
    #[error("OAuth quota response exceeded the size limit")]
    ResponseTooLarge,
    #[error("OAuth quota upstream rejected the request with status {0}")]
    UpstreamRejected(u16),
    #[error("OAuth quota authentication could not be refreshed")]
    AuthenticationFailed,
    #[error("OAuth account has no available quota reset credits")]
    NoResetCredits,
}
