use any2api_runtime::api::{PublicResponse, PublicResponseBody};
use axum::{body::Body, response::Response};

use crate::request_lifecycle::allow_memory_reclamation;

pub(super) fn from_runtime(response: PublicResponse) -> Response {
    let (body, streaming) = match response.body {
        PublicResponseBody::Buffered(body) => (Body::from(body), false),
        PublicResponseBody::Streaming(body) => (Body::from_stream(body), true),
    };
    let mut outgoing = Response::new(body);
    *outgoing.status_mut() = response.status;
    *outgoing.headers_mut() = response.headers;
    if streaming {
        allow_memory_reclamation(&mut outgoing);
    }
    outgoing
}

#[cfg(test)]
mod tests {
    use std::io;

    use any2api_runtime::api::{PublicResponse, PublicResponseBody};
    use axum::{
        body::Bytes,
        http::{HeaderMap, StatusCode},
    };
    use futures_util::stream;

    use super::from_runtime;
    use crate::request_lifecycle::allows_memory_reclamation;

    #[test]
    fn streaming_response_releases_only_the_memory_reclamation_blocker() {
        let response = from_runtime(PublicResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: PublicResponseBody::Streaming(Box::pin(
                stream::empty::<Result<Bytes, io::Error>>(),
            )),
        });

        assert!(allows_memory_reclamation(&response));
    }

    #[test]
    fn buffered_response_keeps_the_default_full_body_lifecycle() {
        let response = from_runtime(PublicResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: PublicResponseBody::Buffered(Bytes::from_static(b"ok")),
        });

        assert!(!allows_memory_reclamation(&response));
    }
}
