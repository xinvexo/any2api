use std::{
    net::SocketAddr,
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use any2api_domain::{HttpHeader, HttpProtocolVersion, RequestId};
use axum::{
    body::Body,
    extract::{ConnectInfo, Request, State},
    http::{HeaderMap, HeaderValue, Version, header::HeaderName},
    middleware::Next,
    response::Response,
};

use crate::{client_address::ClientAddressContext, state::AppState};

use super::{
    body::{AccessLogBody, AccessLogCompletion, AccessLogMetadata},
    capture::{RequestBodyCaptureSlot, RequestCaptureBody},
};

const REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");
const ANY2API_REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-any2api-request-id");

#[derive(Clone, Copy)]
pub(crate) struct HttpRequestId(RequestId);

#[derive(Clone, Copy)]
pub(crate) struct ExcludeFromHttpAccessLog;

#[derive(Clone, Copy)]
pub(crate) struct GatewayAuthRejected;

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
    let snapshot = state.snapshots().load();
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(address)| *address);
    let client_context =
        ClientAddressContext::capture(Arc::clone(&snapshot), peer, request.headers());
    // Capture is skipped entirely when logging cannot persist anything.
    // `should_record` still runs at completion because its status and outcome
    // inputs are unknown here, so it can never pre-exclude a request.
    let mut completion = None;
    if state.request_telemetry().http_access_capture_enabled(
        client_context.snapshot().revision(),
        client_context.snapshot().settings().logging(),
    ) {
        let request_body_capture = RequestBodyCaptureSlot::new();
        completion = Some(AccessLogCompletion::new(
            state.request_telemetry_handle(),
            client_context.snapshot().settings().logging().clone(),
            AccessLogMetadata::new(
                request_id,
                unix_time_ms(),
                client_context.snapshot().revision(),
                client_context.audit_client_ip(),
                request.method().as_str().to_owned(),
                request.uri().path().to_owned(),
                request.uri().to_string(),
                protocol_version(request.version()),
                capture_headers(request.headers()),
                request_body_capture.clone(),
            ),
            Instant::now(),
        ));
        request =
            request.map(|body| Body::new(RequestCaptureBody::new(body, request_body_capture)));
    }
    request.extensions_mut().insert(client_context);
    request.extensions_mut().insert(HttpRequestId(request_id));

    let mut response = next.run(request).await;
    let mut uuid_buffer = [0_u8; uuid::fmt::Hyphenated::LENGTH];
    let request_id_value = HeaderValue::from_str(
        request_id
            .as_uuid()
            .hyphenated()
            .encode_lower(&mut uuid_buffer),
    )
    .expect("request UUID is a valid header");
    response
        .headers_mut()
        .insert(ANY2API_REQUEST_ID_HEADER, request_id_value.clone());
    if !response.headers().contains_key(&REQUEST_ID_HEADER) {
        response
            .headers_mut()
            .insert(REQUEST_ID_HEADER, request_id_value);
    }
    let Some(mut completion) = completion else {
        return response;
    };
    completion.set_response(
        response.status().as_u16(),
        capture_headers(response.headers()),
    );
    if response
        .extensions_mut()
        .remove::<GatewayAuthRejected>()
        .is_some()
    {
        completion.mark_gateway_auth_rejected();
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

fn capture_headers(headers: &HeaderMap) -> Vec<HttpHeader> {
    headers
        .iter()
        .map(|(name, value)| HttpHeader {
            name: name.as_str().to_owned(),
            value: value.as_bytes().to_vec(),
        })
        .collect()
}

fn protocol_version(version: Version) -> HttpProtocolVersion {
    match version {
        Version::HTTP_09 => HttpProtocolVersion::Http09,
        Version::HTTP_10 => HttpProtocolVersion::Http10,
        Version::HTTP_11 => HttpProtocolVersion::Http11,
        Version::HTTP_2 => HttpProtocolVersion::Http2,
        Version::HTTP_3 => HttpProtocolVersion::Http3,
        // `http::Version` is non-exhaustive. Keep access logging available if a
        // future dependency version adds a protocol before the persisted
        // telemetry model learns its exact spelling.
        _ => HttpProtocolVersion::Http11,
    }
}

fn unix_time_ms() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    u64::try_from(millis).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_versions_map_to_the_persisted_telemetry_values() {
        assert_eq!(
            protocol_version(Version::HTTP_09),
            HttpProtocolVersion::Http09
        );
        assert_eq!(
            protocol_version(Version::HTTP_10),
            HttpProtocolVersion::Http10
        );
        assert_eq!(
            protocol_version(Version::HTTP_11),
            HttpProtocolVersion::Http11
        );
        assert_eq!(
            protocol_version(Version::HTTP_2),
            HttpProtocolVersion::Http2
        );
        assert_eq!(
            protocol_version(Version::HTTP_3),
            HttpProtocolVersion::Http3
        );
    }
}
