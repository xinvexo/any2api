use std::fmt;

use serde::{Deserialize, Serialize};

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
    PayloadTooLarge,
    PublicApiNotFound,
    MethodNotAllowed,
    ModelNotFound,
    NoRoute,
    NoAvailableCredential,
    LocalRateLimit,
    SessionBindingLost,
    UpstreamNotFound,
    UpstreamRateLimit,
    UpstreamOverloaded,
    UpstreamError,
    InternalError,
}

#[derive(Clone, Eq, PartialEq)]
pub struct PublicError {
    code: PublicErrorCode,
    message: PublicErrorMessage,
    retry_after_seconds: Option<u64>,
}

#[derive(Clone, Eq, PartialEq)]
enum PublicErrorMessage {
    Local(String),
    Provider(String),
}

impl PublicError {
    #[must_use]
    pub fn new(code: PublicErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: PublicErrorMessage::Local(message.into()),
            retry_after_seconds: None,
        }
    }

    #[must_use]
    pub fn from_provider_message(code: PublicErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: PublicErrorMessage::Provider(message.into()),
            retry_after_seconds: None,
        }
    }

    #[must_use]
    pub const fn code(&self) -> PublicErrorCode {
        self.code
    }

    #[must_use]
    pub fn client_message(&self) -> &str {
        match &self.message {
            PublicErrorMessage::Local(message) | PublicErrorMessage::Provider(message) => message,
        }
    }

    #[must_use]
    pub fn telemetry_message(&self) -> &str {
        match &self.message {
            PublicErrorMessage::Local(message) => message,
            PublicErrorMessage::Provider(_) => "upstream request failed",
        }
    }

    #[must_use]
    pub const fn retry_after_seconds(&self) -> Option<u64> {
        self.retry_after_seconds
    }

    #[must_use]
    pub const fn with_retry_after_seconds(mut self, seconds: u64) -> Self {
        self.retry_after_seconds = Some(seconds);
        self
    }
}

impl fmt::Debug for PublicError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message_source = match self.message {
            PublicErrorMessage::Local(_) => "local",
            PublicErrorMessage::Provider(_) => "provider",
        };
        formatter
            .debug_struct("PublicError")
            .field("code", &self.code)
            .field("message_source", &message_source)
            .field("retry_after_seconds", &self.retry_after_seconds)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_message_is_client_visible_but_not_a_telemetry_or_debug_message() {
        let error = PublicError::from_provider_message(
            PublicErrorCode::UpstreamError,
            "official provider detail",
        );

        assert_eq!(error.client_message(), "official provider detail");
        assert_eq!(error.telemetry_message(), "upstream request failed");
        assert!(!format!("{error:?}").contains("official provider detail"));
    }
}
