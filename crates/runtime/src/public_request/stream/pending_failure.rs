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
                failure_scope: TransportFailureScope::Endpoint,
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
}

#[derive(Clone, Copy)]
pub(super) enum PendingStreamErrorKind {
    Transport {
        retry_safety: RetrySafety,
        failure_scope: TransportFailureScope,
    },
    InvalidResponse,
    Local,
}

impl PendingStreamErrorKind {
    pub(super) fn error_class(self) -> ErrorClass {
        match self {
            Self::Transport { failure_scope, .. } => transport_error_class(failure_scope),
            Self::InvalidResponse => ErrorClass::Upstream,
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
    use any2api_domain::ErrorClass;
    use any2api_transport::api::TransportFailureScope;

    use super::transport_error_class;

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
}
