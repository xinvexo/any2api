use std::{
    net::SocketAddr,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use any2api_domain::{
    HttpProtocolVersion, MAX_HTTP_ACCESS_LOG_METHOD_CHARS, MAX_HTTP_ACCESS_LOG_PATH_CHARS,
    RequestId, bounded_log_text,
};
use axum::{
    body::Body,
    extract::{ConnectInfo, Request, State},
    http::{HeaderValue, Version, header::HeaderName},
    middleware::Next,
    response::Response,
};

use crate::state::AppState;

use super::body::{AccessLogBody, AccessLogCompletion, AccessLogMetadata};

const REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");
const ANY2API_REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-any2api-request-id");

#[derive(Clone, Copy)]
pub(crate) struct HttpRequestId(RequestId);

#[derive(Clone, Copy)]
pub(crate) struct ExcludeFromHttpAccessLog;

impl HttpRequestId {
    pub(crate) const fn get(self) -> RequestId {
        self.0
    }
}

pub(crate) async fn record(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let request_id = RequestId::new();
    let started = Instant::now();
    let started_at_ms = unix_time_ms();
    let snapshot = state.snapshots().load();
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(address)| *address);
    let client_ip = state
        .client_addresses()
        .resolve(peer, request.headers())
        .map(|connection| connection.client_ip())
        .ok()
        .or_else(|| peer.map(|address| address.ip()));
    let mut completion = AccessLogCompletion::new(
        state.request_telemetry_handle(),
        snapshot.settings().logging().clone(),
        AccessLogMetadata::new(
            request_id,
            started_at_ms,
            snapshot.revision(),
            client_ip,
            bounded_log_text(request.method().as_str(), MAX_HTTP_ACCESS_LOG_METHOD_CHARS)
                .to_owned(),
            bounded_log_text(request.uri().path(), MAX_HTTP_ACCESS_LOG_PATH_CHARS).to_owned(),
            protocol_version(request.version()),
        ),
        started,
    );
    request.extensions_mut().insert(HttpRequestId(request_id));

    let mut response = next.run(request).await;
    completion.set_status(response.status().as_u16());
    let request_id_value =
        HeaderValue::from_str(&request_id.to_string()).expect("request UUID is a valid header");
    response
        .headers_mut()
        .insert(ANY2API_REQUEST_ID_HEADER, request_id_value.clone());
    if !response.headers().contains_key(&REQUEST_ID_HEADER) {
        response
            .headers_mut()
            .insert(REQUEST_ID_HEADER, request_id_value);
    }
    if response
        .extensions_mut()
        .remove::<ExcludeFromHttpAccessLog>()
        .is_some()
    {
        completion.exclude();
        return response;
    }
    response.map(|body| Body::new(AccessLogBody::new(body, completion)))
}

fn protocol_version(version: Version) -> HttpProtocolVersion {
    match version {
        Version::HTTP_09 => HttpProtocolVersion::Http09,
        Version::HTTP_10 => HttpProtocolVersion::Http10,
        Version::HTTP_11 => HttpProtocolVersion::Http11,
        Version::HTTP_2 => HttpProtocolVersion::Http2,
        Version::HTTP_3 => HttpProtocolVersion::Http3,
        _ => unreachable!("http crate returned an unknown HTTP version"),
    }
}

fn unix_time_ms() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    u64::try_from(millis).unwrap_or(u64::MAX)
}
