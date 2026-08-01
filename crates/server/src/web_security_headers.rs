use axum::{
    extract::Request,
    http::{HeaderMap, HeaderValue, header::HeaderName},
    middleware::Next,
    response::Response,
};

const CONTENT_SECURITY_POLICY: HeaderName = HeaderName::from_static("content-security-policy");
const X_CONTENT_TYPE_OPTIONS: HeaderName = HeaderName::from_static("x-content-type-options");
const REFERRER_POLICY: HeaderName = HeaderName::from_static("referrer-policy");

const POLICY: &str = "default-src 'self'; base-uri 'none'; object-src 'none'; frame-ancestors 'none'; form-action 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; connect-src 'self'; manifest-src 'self'";

pub(crate) async fn add(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    insert(response.headers_mut());
    response
}

pub(crate) fn insert(headers: &mut HeaderMap) {
    headers.insert(CONTENT_SECURITY_POLICY, HeaderValue::from_static(POLICY));
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
}
