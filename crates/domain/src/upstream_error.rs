use std::{
    fmt,
    time::{Duration, SystemTime},
};

use serde::{Deserialize, Serialize};

use crate::{ErrorClass, RetrySafety};

pub const MAX_RETRY_AFTER_SECONDS: u64 = 30 * 24 * 60 * 60;
pub const MAX_UPSTREAM_ERROR_MESSAGE_BYTES: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpstreamErrorKind {
    InvalidRequest,
    Authentication,
    PermissionDenied,
    QuotaExhausted,
    RateLimited,
    ModelUnavailable,
    OperationUnavailable,
    Transient,
    Unknown,
}

impl UpstreamErrorKind {
    #[must_use]
    pub const fn error_class(self) -> ErrorClass {
        match self {
            Self::InvalidRequest => ErrorClass::InvalidRequest,
            Self::Authentication => ErrorClass::Authentication,
            Self::PermissionDenied => ErrorClass::PermissionDenied,
            Self::QuotaExhausted => ErrorClass::QuotaExhausted,
            Self::RateLimited => ErrorClass::RateLimited,
            Self::ModelUnavailable => ErrorClass::ModelUnavailable,
            Self::OperationUnavailable => ErrorClass::OperationUnavailable,
            Self::Transient | Self::Unknown => ErrorClass::Upstream,
        }
    }

    #[must_use]
    pub const fn is_retry_candidate(self) -> bool {
        matches!(
            self,
            Self::PermissionDenied
                | Self::QuotaExhausted
                | Self::RateLimited
                | Self::ModelUnavailable
                | Self::Transient
        )
    }

    #[must_use]
    pub const fn default_retry_safety(self) -> RetrySafety {
        match self {
            Self::Authentication
            | Self::PermissionDenied
            | Self::QuotaExhausted
            | Self::RateLimited
            | Self::ModelUnavailable
            | Self::OperationUnavailable => RetrySafety::RejectedBeforeExecution,
            Self::InvalidRequest | Self::Transient | Self::Unknown => RetrySafety::Ambiguous,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetryAfterHint {
    Delay(Duration),
    At(SystemTime),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UpstreamQuotaExhaustion {
    used: u64,
    limit: u64,
}

impl UpstreamQuotaExhaustion {
    #[must_use]
    pub const fn new(used: u64, limit: u64) -> Self {
        Self { used, limit }
    }

    #[must_use]
    pub const fn used(self) -> u64 {
        self.used
    }

    #[must_use]
    pub const fn limit(self) -> u64 {
        self.limit
    }
}

impl RetryAfterHint {
    #[must_use]
    pub fn delay_from(self, now: SystemTime) -> Duration {
        let delay = match self {
            Self::Delay(delay) => delay,
            Self::At(instant) => instant.duration_since(now).unwrap_or_default(),
        };
        delay.min(Duration::from_secs(MAX_RETRY_AFTER_SECONDS))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UpstreamErrorClassification {
    kind: UpstreamErrorKind,
    retry_safety: RetrySafety,
    retry_after: Option<RetryAfterHint>,
    quota_exhaustion: Option<UpstreamQuotaExhaustion>,
}

impl UpstreamErrorClassification {
    #[must_use]
    pub const fn new(
        kind: UpstreamErrorKind,
        retry_safety: RetrySafety,
        retry_after: Option<RetryAfterHint>,
    ) -> Self {
        Self {
            kind,
            retry_safety,
            retry_after,
            quota_exhaustion: None,
        }
    }

    #[must_use]
    pub const fn with_quota_exhaustion(mut self, observation: UpstreamQuotaExhaustion) -> Self {
        self.quota_exhaustion = Some(observation);
        self
    }

    #[must_use]
    pub const fn kind(self) -> UpstreamErrorKind {
        self.kind
    }

    #[must_use]
    pub const fn retry_safety(self) -> RetrySafety {
        self.retry_safety
    }

    #[must_use]
    pub const fn retry_after(self) -> Option<RetryAfterHint> {
        self.retry_after
    }

    #[must_use]
    pub const fn quota_exhaustion(self) -> Option<UpstreamQuotaExhaustion> {
        self.quota_exhaustion
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct UpstreamError {
    classification: UpstreamErrorClassification,
    official_message: Option<String>,
}

impl UpstreamError {
    #[must_use]
    pub fn new(
        classification: UpstreamErrorClassification,
        official_message: Option<String>,
    ) -> Self {
        Self {
            classification,
            official_message: official_message.and_then(normalize_message),
        }
    }

    #[must_use]
    pub const fn classification(&self) -> UpstreamErrorClassification {
        self.classification
    }

    #[must_use]
    pub fn official_message(&self) -> Option<&str> {
        self.official_message.as_deref()
    }
}

impl fmt::Debug for UpstreamError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UpstreamError")
            .field("classification", &self.classification)
            .field("official_message_present", &self.official_message.is_some())
            .finish()
    }
}

fn normalize_message(message: String) -> Option<String> {
    let trimmed = message.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_UPSTREAM_ERROR_MESSAGE_BYTES {
        return None;
    }
    if trimmed.len() == message.len() {
        Some(message)
    } else {
        Some(trimmed.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_upstream_error_kind_has_one_default_retry_safety() {
        for kind in [
            UpstreamErrorKind::InvalidRequest,
            UpstreamErrorKind::Authentication,
            UpstreamErrorKind::PermissionDenied,
            UpstreamErrorKind::QuotaExhausted,
            UpstreamErrorKind::RateLimited,
            UpstreamErrorKind::ModelUnavailable,
            UpstreamErrorKind::OperationUnavailable,
            UpstreamErrorKind::Transient,
            UpstreamErrorKind::Unknown,
        ] {
            let expected = match kind {
                UpstreamErrorKind::Authentication
                | UpstreamErrorKind::PermissionDenied
                | UpstreamErrorKind::QuotaExhausted
                | UpstreamErrorKind::RateLimited
                | UpstreamErrorKind::ModelUnavailable
                | UpstreamErrorKind::OperationUnavailable => RetrySafety::RejectedBeforeExecution,
                UpstreamErrorKind::InvalidRequest
                | UpstreamErrorKind::Transient
                | UpstreamErrorKind::Unknown => RetrySafety::Ambiguous,
            };

            assert_eq!(kind.default_retry_safety(), expected, "kind {kind:?}");
            assert_eq!(
                kind.default_retry_safety().allows_automatic_retry(),
                !matches!(expected, RetrySafety::Ambiguous),
                "kind {kind:?}"
            );
        }
    }

    fn classification() -> UpstreamErrorClassification {
        UpstreamErrorClassification::new(UpstreamErrorKind::Unknown, RetrySafety::Ambiguous, None)
    }

    #[test]
    fn official_message_is_trimmed_and_kept_separate_from_classification() {
        let error = UpstreamError::new(
            classification(),
            Some("  official provider detail  ".to_owned()),
        );

        assert_eq!(error.classification(), classification());
        assert_eq!(error.official_message(), Some("official provider detail"));
        assert!(!format!("{error:?}").contains("official provider detail"));
    }

    #[test]
    fn empty_or_oversized_messages_are_not_public() {
        assert_eq!(
            UpstreamError::new(classification(), Some(" \n ".to_owned())).official_message(),
            None
        );
        assert_eq!(
            UpstreamError::new(
                classification(),
                Some("x".repeat(MAX_UPSTREAM_ERROR_MESSAGE_BYTES + 1)),
            )
            .official_message(),
            None
        );
    }
}
