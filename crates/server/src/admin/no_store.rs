use axum::{
    Json,
    extract::Request,
    http::{
        HeaderValue,
        header::{CACHE_CONTROL, VARY},
    },
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::Serialize;

pub(crate) fn json<T>(value: T) -> Response
where
    T: Serialize,
{
    let mut response = Json(value).into_response();
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

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
