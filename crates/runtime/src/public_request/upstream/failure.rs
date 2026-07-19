use any2api_domain::{PublicError, RetrySafety, UpstreamErrorClassification};
use any2api_transport::api::{TransportError, TransportFailureScope};

use crate::route_candidates::RouteCandidate;

use super::super::response::{classified_error, public_error};

pub(in crate::public_request) enum AttemptFailure {
    Transport {
        error: Box<TransportError>,
        candidate: Box<RouteCandidate>,
        fixed: bool,
    },
    Upstream {
        classification: UpstreamErrorClassification,
        candidate: Box<RouteCandidate>,
        fixed: bool,
    },
    Public(PublicError),
}

impl AttemptFailure {
    pub(super) fn transport(error: TransportError, candidate: RouteCandidate, fixed: bool) -> Self {
        Self::Transport {
            error: Box::new(error),
            candidate: Box::new(candidate),
            fixed,
        }
    }

    pub(super) fn upstream(
        classification: UpstreamErrorClassification,
        candidate: RouteCandidate,
        fixed: bool,
    ) -> Self {
        Self::Upstream {
            classification,
            candidate: Box::new(candidate),
            fixed,
        }
    }

    pub(in crate::public_request) fn public_error(&self) -> PublicError {
        match self {
            Self::Transport { .. } => public_error(
                any2api_domain::PublicErrorCode::UpstreamError,
                "upstream request failed",
            ),
            Self::Upstream { classification, .. } => classified_error(*classification),
            Self::Public(error) => error.clone(),
        }
    }

    pub(in crate::public_request) fn retry_safety(&self) -> RetrySafety {
        match self {
            Self::Transport { error, .. } => error.retry_safety,
            Self::Upstream { classification, .. } => classification.retry_safety(),
            Self::Public(_) => RetrySafety::Ambiguous,
        }
    }

    pub(in crate::public_request) fn is_retry_candidate(&self) -> bool {
        match self {
            Self::Transport { .. } => true,
            Self::Upstream { classification, .. } => classification.kind().is_retry_candidate(),
            Self::Public(_) => false,
        }
    }

    pub(in crate::public_request) fn candidate(&self) -> Option<&RouteCandidate> {
        match self {
            Self::Transport { candidate, .. } | Self::Upstream { candidate, .. } => {
                Some(candidate.as_ref())
            }
            Self::Public(_) => None,
        }
    }

    pub(in crate::public_request) fn fixed(&self) -> bool {
        match self {
            Self::Transport { fixed, .. } | Self::Upstream { fixed, .. } => *fixed,
            Self::Public(_) => true,
        }
    }

    pub(in crate::public_request) fn transport_failure_scope(
        &self,
    ) -> Option<TransportFailureScope> {
        match self {
            Self::Transport { error, .. } => Some(error.failure_scope),
            Self::Upstream { .. } | Self::Public(_) => None,
        }
    }
}
