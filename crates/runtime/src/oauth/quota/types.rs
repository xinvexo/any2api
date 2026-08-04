//! Provider-neutral OAuth quota results exposed by Runtime.

use std::sync::Arc;

use any2api_provider::api::{OAuthQuotaUsage, ProviderError};
use any2api_storage::api::StorageError;
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

#[derive(Clone, Debug, Error)]
pub enum OAuthQuotaError {
    #[error("OAuth account was not found")]
    AccountNotFound,
    #[error("OAuth provider driver is unavailable")]
    ProviderUnavailable,
    #[error("OAuth quota is not supported by this provider")]
    UnsupportedProvider,
    #[error("OAuth account runtime is unavailable")]
    RuntimeUnavailable,
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
    #[error("OAuth account access is restricted by the upstream provider")]
    AccountRestricted,
    #[error("the OAuth provider rejected the current network egress")]
    ProviderEgressRestricted,
    #[error("OAuth account authentication could not be verified because token refresh failed")]
    AuthenticationRefreshFailed,
    #[error("OAuth account was rejected after token refresh")]
    AuthenticationFailed,
    #[error("OAuth account has no available quota reset credits")]
    NoResetCredits,
    #[error("OAuth quota persistence failed")]
    Persistence(#[source] Arc<StorageError>),
    #[error("persisted OAuth quota snapshot is invalid")]
    InvalidPersistedSnapshot,
}
