use std::fmt;

use any2api_domain::RetrySafety;
use bytes::Bytes;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolRetryDelayBasis {
    CandidateAttempts,
    RequestAttempts,
}

/// Complete, protocol-validated upstream failure JSON. Constructors are kept
/// explicit so callers cannot infer failure evidence from arbitrary prose.
#[derive(Clone, Eq, PartialEq)]
pub struct ProtocolUpstreamFailureEvidence {
    raw_json: Bytes,
    retry_safety_override: Option<RetrySafety>,
    retry_delay_basis: ProtocolRetryDelayBasis,
}

impl ProtocolUpstreamFailureEvidence {
    #[must_use]
    pub fn new(raw_json: Bytes) -> Self {
        Self {
            raw_json,
            retry_safety_override: None,
            retry_delay_basis: ProtocolRetryDelayBasis::CandidateAttempts,
        }
    }

    #[must_use]
    pub const fn with_retry_safety_override(mut self, safety: RetrySafety) -> Self {
        self.retry_safety_override = Some(safety);
        self
    }

    #[must_use]
    pub const fn with_retry_delay_basis(mut self, basis: ProtocolRetryDelayBasis) -> Self {
        self.retry_delay_basis = basis;
        self
    }

    #[must_use]
    pub fn raw_json(&self) -> &Bytes {
        &self.raw_json
    }

    #[must_use]
    pub const fn retry_safety_override(&self) -> Option<RetrySafety> {
        self.retry_safety_override
    }

    #[must_use]
    pub const fn retry_delay_basis(&self) -> ProtocolRetryDelayBasis {
        self.retry_delay_basis
    }
}

impl fmt::Debug for ProtocolUpstreamFailureEvidence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProtocolUpstreamFailureEvidence")
            .field("raw_json_bytes", &self.raw_json.len())
            .field("retry_safety_override", &self.retry_safety_override)
            .field("retry_delay_basis", &self.retry_delay_basis)
            .finish()
    }
}
