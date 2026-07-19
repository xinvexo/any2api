use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::{ErrorClass, RetrySafety};

pub const MAX_RETRY_AFTER_SECONDS: u64 = 30 * 24 * 60 * 60;

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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetryAfterHint {
    Delay(Duration),
    At(SystemTime),
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
        }
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
}
