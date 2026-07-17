use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorClass {
    InvalidRequest,
    Authentication,
    PermissionDenied,
    RateLimited,
    ModelUnavailable,
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
    UpstreamError,
    InternalError,
}

#[derive(Clone, Debug, Error, Eq, PartialEq, Serialize)]
#[error("{code:?}: {message}")]
pub struct PublicError {
    pub code: PublicErrorCode,
    pub message: String,
}

impl PublicError {
    #[must_use]
    pub fn new(code: PublicErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
