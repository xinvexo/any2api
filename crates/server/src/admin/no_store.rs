use axum::{
    extract::Request,
    http::{
        HeaderValue,
        header::{CACHE_CONTROL, VARY},
    },
    middleware::Next,
    response::Response,
};

pub(crate) async fn responses(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
        .headers_mut()
        .append(VARY, HeaderValue::from_static("Cookie"));
    response
}
