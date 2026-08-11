use any2api_domain::{ErrorClass, PublicError, UpstreamError};
use any2api_protocol::api::{EgressResponse, StreamRejection, StreamRetryReason};
use bytes::Bytes;
use http::{HeaderMap, StatusCode};

pub(in crate::public_request) enum FinalFailure {
    Local {
        error: PublicError,
    },
    Upstream {
        response: EgressResponse,
        error_class: ErrorClass,
        error_message: Option<String>,
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
            error_message: upstream.official_message().map(ToOwned::to_owned),
        }
    }

    pub(in crate::public_request) fn stream_rejection(
        headers: HeaderMap,
        status: StatusCode,
        body: Bytes,
        rejection: StreamRejection,
    ) -> Self {
        let error_class = match rejection.reason() {
            StreamRetryReason::Overloaded => ErrorClass::Upstream,
            StreamRetryReason::RateLimited => ErrorClass::RateLimited,
        };
        Self::Upstream {
            response: EgressResponse {
                status,
                headers,
                body,
            },
            error_class,
            error_message: Some(rejection.code().to_owned()),
        }
    }
}

impl From<PublicError> for FinalFailure {
    fn from(error: PublicError) -> Self {
        Self::Local { error }
    }
}

#[cfg(test)]
mod tests {
    use any2api_domain::ErrorClass;
    use any2api_protocol::api::{StreamRejection, StreamRetryReason};
    use bytes::Bytes;
    use http::{HeaderMap, StatusCode};

    use super::FinalFailure;

    #[test]
    fn stream_rejection_keeps_the_real_status_frame_and_stable_code() {
        let frame = Bytes::from_static(
            b"event: error\ndata: {\"type\":\"error\",\"error\":{\"code\":\"server_is_overloaded\"}}\n\n",
        );
        let FinalFailure::Upstream {
            response,
            error_class,
            error_message,
        } = FinalFailure::stream_rejection(
            HeaderMap::new(),
            StatusCode::OK,
            frame.clone(),
            StreamRejection::new(StreamRetryReason::Overloaded, "server_is_overloaded"),
        )
        else {
            panic!("stream rejection must remain an upstream response");
        };

        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(response.body, frame);
        assert_eq!(error_class, ErrorClass::Upstream);
        assert_eq!(error_message.as_deref(), Some("server_is_overloaded"));
    }
}
