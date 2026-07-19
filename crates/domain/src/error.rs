use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorClass {
    InvalidRequest,
    Authentication,
    PermissionDenied,
    QuotaExhausted,
    RateLimited,
    ModelUnavailable,
    OperationUnavailable,
    Proxy,
    Network,
    Upstream,
    Cancelled,
    Internal,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicErrorCode {
    Unauthorized,
    InvalidRequest,
    ModelNotFound,
    NoRoute,
    NoAvailableCredential,
    LocalConcurrencyLimit,
    SessionBindingLost,
    UpstreamNotFound,
    UpstreamError,
    InternalError,
}

#[derive(Clone, Debug, Error, Eq, PartialEq, Serialize)]
#[error("{code:?}: {message}")]
pub struct PublicError {
    pub code: PublicErrorCode,
    pub message: String,
    pub retry_after_seconds: Option<u64>,
}

impl PublicError {
    #[must_use]
    pub fn new(code: PublicErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            retry_after_seconds: None,
        }
    }

    #[must_use]
    pub const fn with_retry_after_seconds(mut self, seconds: u64) -> Self {
        self.retry_after_seconds = Some(seconds);
        self
    }
}
