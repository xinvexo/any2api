use any2api_domain::{ErrorClass, PublicError, UpstreamError};
use any2api_protocol::api::EgressResponse;
use bytes::Bytes;
use http::{HeaderMap, StatusCode};

pub(in crate::public_request) enum FinalFailure {
    Local {
        error: PublicError,
    },
    Upstream {
        response: EgressResponse,
        error_class: ErrorClass,
    },
}

impl FinalFailure {
    pub(in crate::public_request) fn upstream(
        headers: HeaderMap,
        status: StatusCode,
        body: Bytes,
        upstream: &UpstreamError,
    ) -> Self {
        Self::Upstream {
            response: EgressResponse {
                status,
                headers,
                body,
            },
            error_class: upstream.classification().kind().error_class(),
        }
    }
}

impl From<PublicError> for FinalFailure {
    fn from(error: PublicError) -> Self {
        Self::Local { error }
    }
}
