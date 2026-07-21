use any2api_runtime::api::{PublicResponse, PublicResponseBody};
use axum::{body::Body, response::Response};

pub(super) fn from_runtime(response: PublicResponse) -> Response {
    let body = match response.body {
        PublicResponseBody::Buffered(body) => Body::from(body),
        PublicResponseBody::Streaming(body) => Body::from_stream(body),
    };
    let mut outgoing = Response::new(body);
    *outgoing.status_mut() = response.status;
    *outgoing.headers_mut() = response.headers;
    outgoing
}
