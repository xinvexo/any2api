use std::net::SocketAddr;

use axum::{
    extract::{ConnectInfo, Request},
    middleware::Next,
    response::{IntoResponse, Response},
};

use super::error::AdminApiError;

pub(crate) async fn require_loopback(request: Request, next: Next) -> Response {
    let is_loopback = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .is_some_and(|ConnectInfo(address)| address.ip().is_loopback());

    if is_loopback {
        next.run(request).await
    } else {
        AdminApiError::loopback_only().into_response()
    }
}
