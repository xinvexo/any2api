use std::io;

use any2api_domain::{ErrorClass, RetrySafety};
use any2api_transport::api::{TransportError, TransportFailureScope};

pub(super) struct PendingStreamError {
    pub(super) error: io::Error,
    pub(super) kind: PendingStreamErrorKind,
}

impl PendingStreamError {
    pub(super) fn transport(error: &TransportError) -> Self {
        Self {
            error: stream_error("upstream stream failed"),
            kind: PendingStreamErrorKind::Transport {
                retry_safety: error.retry_safety,
                failure_scope: error.failure_scope,
            },
        }
    }

    pub(super) fn timeout() -> Self {
        Self {
            error: stream_error("upstream stream precommit timed out"),
            kind: PendingStreamErrorKind::Transport {
                retry_safety: RetrySafety::Ambiguous,
                failure_scope: TransportFailureScope::Unattributed,
            },
        }
    }

    pub(super) fn invalid_response(message: &'static str) -> Self {
        Self {
            error: stream_error(message),
            kind: PendingStreamErrorKind::InvalidResponse,
        }
    }

    pub(super) fn local(message: &'static str) -> Self {
        Self {
            error: stream_error(message),
            kind: PendingStreamErrorKind::Local,
        }
    }

    pub(super) fn budget_exceeded() -> Self {
        Self {
            error: stream_error("upstream stream exceeded the precommit byte budget"),
            kind: PendingStreamErrorKind::BudgetExceeded,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PendingStreamErrorKind {
    Transport {
        retry_safety: RetrySafety,
        failure_scope: TransportFailureScope,
    },
    InvalidResponse,
    BudgetExceeded,
    Local,
}

impl PendingStreamErrorKind {
    pub(super) fn error_class(self) -> ErrorClass {
        match self {
            Self::Transport { failure_scope, .. } => transport_error_class(failure_scope),
            Self::InvalidResponse | Self::BudgetExceeded => ErrorClass::Upstream,
            Self::Local => ErrorClass::Internal,
        }
    }
}

pub(super) fn transport_error_class(failure_scope: TransportFailureScope) -> ErrorClass {
    match failure_scope {
        TransportFailureScope::Proxy => ErrorClass::Proxy,
        TransportFailureScope::Endpoint | TransportFailureScope::Unattributed => {
            ErrorClass::Network
        }
    }
}

fn stream_error(message: &'static str) -> io::Error {
    io::Error::other(message)
}

#[cfg(test)]
mod tests {
    use any2api_domain::{ErrorClass, RetrySafety};
    use any2api_transport::api::TransportFailureScope;

    use super::{PendingStreamError, PendingStreamErrorKind, transport_error_class};

    #[test]
    fn transport_error_class_preserves_proxy_attribution() {
        assert_eq!(
            transport_error_class(TransportFailureScope::Proxy),
            ErrorClass::Proxy
        );
        assert_eq!(
            transport_error_class(TransportFailureScope::Endpoint),
            ErrorClass::Network
        );
    }

    #[test]
    fn runtime_precommit_timeout_is_not_attributed_to_endpoint_or_proxy() {
        assert_eq!(
            PendingStreamError::timeout().kind,
            PendingStreamErrorKind::Transport {
                retry_safety: RetrySafety::Ambiguous,
                failure_scope: TransportFailureScope::Unattributed,
            }
        );
    }
}
