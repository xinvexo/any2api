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

impl ErrorClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::Authentication => "authentication",
            Self::PermissionDenied => "permission_denied",
            Self::QuotaExhausted => "quota_exhausted",
            Self::RateLimited => "rate_limited",
            Self::ModelUnavailable => "model_unavailable",
            Self::OperationUnavailable => "operation_unavailable",
            Self::Proxy => "proxy",
            Self::Network => "network",
            Self::Upstream => "upstream",
            Self::Cancelled => "cancelled",
            Self::Internal => "internal",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "invalid_request" => Some(Self::InvalidRequest),
            "authentication" => Some(Self::Authentication),
            "permission_denied" => Some(Self::PermissionDenied),
            "quota_exhausted" => Some(Self::QuotaExhausted),
            "rate_limited" => Some(Self::RateLimited),
            "model_unavailable" => Some(Self::ModelUnavailable),
            "operation_unavailable" => Some(Self::OperationUnavailable),
            "proxy" => Some(Self::Proxy),
            "network" => Some(Self::Network),
            "upstream" => Some(Self::Upstream),
            "cancelled" => Some(Self::Cancelled),
            "internal" => Some(Self::Internal),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicErrorCode {
    Unauthorized,
    InvalidRequest,
    PublicApiNotFound,
    MethodNotAllowed,
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
