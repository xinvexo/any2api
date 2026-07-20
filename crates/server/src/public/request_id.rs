use any2api_domain::RequestId;
use axum::{
    extract::Request,
    http::{HeaderValue, header::HeaderName},
    middleware::Next,
    response::Response,
};

const REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");

#[derive(Clone, Copy)]
pub(crate) struct PublicRequestId(RequestId);

impl PublicRequestId {
    pub(crate) const fn get(self) -> RequestId {
        self.0
    }
}

pub(crate) async fn assign(mut request: Request, next: Next) -> Response {
    let request_id = RequestId::new();
    request.extensions_mut().insert(PublicRequestId(request_id));
    let mut response = next.run(request).await;
    if let Ok(value) = HeaderValue::from_str(&request_id.to_string()) {
        response.headers_mut().insert(REQUEST_ID_HEADER, value);
    }
    response
}
