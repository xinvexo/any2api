//! Provider-neutral OAuth quota results exposed by Runtime.

use std::sync::Arc;

use any2api_provider::api::{OAuthQuotaUsage, ProviderError};
use any2api_storage::api::StorageError;
use any2api_transport::api::TransportError;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::oauth::refresh::OAuthRefreshFailure;

#[derive(Clone, Debug, PartialEq)]
pub struct OAuthQuotaSnapshot {
    pub usage: OAuthQuotaUsage,
    pub usd_estimates: Vec<OAuthQuotaUsdEstimate>,
    pub fetched_at: i64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct OAuthQuotaUsdEstimate {
    pub window_id: String,
    pub window_kind: any2api_provider::api::OAuthQuotaWindowKind,
    pub limit_window_seconds: Option<u64>,
    pub window_reset_at: Option<i64>,
    pub estimated_capacity_usd: f64,
    pub estimated_used_usd: f64,
    pub estimated_remaining_usd: f64,
    pub sample_cost_usd: f64,
    pub sample_used_percent_delta: f64,
    pub sample_started_at: i64,
    pub sample_ended_at: i64,
    pub pricing_basis: String,
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
    #[error("OAuth account has no refresh token after its access token was rejected")]
    RefreshTokenMissing(OAuthRefreshFailure),
    #[error("OAuth refresh endpoint permanently rejected the refresh token")]
    RefreshPermanentlyRejected(OAuthRefreshFailure),
    #[error("OAuth token refresh failed")]
    TokenRefreshFailed(OAuthRefreshFailure),
    #[error("OAuth account was rejected after a successful token refresh")]
    RefreshedAccessTokenRejected(OAuthRefreshFailure),
    #[error("OAuth account has no available quota reset credits")]
    NoResetCredits,
    #[error("OAuth quota persistence failed")]
    Persistence(#[source] Arc<StorageError>),
    #[error("persisted OAuth quota snapshot is invalid")]
    InvalidPersistedSnapshot,
}
