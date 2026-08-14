//! OpenAI Responses WebSocket ingress (ADR-0151).
//!
//! `GET /v1/responses` upgrades into the `responses_websockets=2026-02-06`
//! framing used by Codex CLI. Each `response.create` message replays the
//! existing public request pipeline; the upstream transport stays HTTP.

mod bridge;
mod connection;

use std::sync::atomic::{AtomicUsize, Ordering};

use axum::{
    extract::{Extension, State, WebSocketUpgrade, ws::rejection::WebSocketUpgradeRejection},
    http::{HeaderMap, StatusCode, header::HeaderName},
    response::Response,
};

use crate::state::AppState;

use super::auth::AuthenticatedGatewayApiKey;
use connection::WsConnectionContext;

/// Hard cap on concurrent Responses WebSocket connections. Upgrades over the
/// cap receive HTTP 426, the only status the client treats as a clean signal
/// to fall back to the HTTP transport.
const MAX_CONNECTIONS: usize = 64;

static CONNECTIONS: AtomicUsize = AtomicUsize::new(0);

pub(super) struct ConnectionSlot(());

impl ConnectionSlot {
    fn acquire() -> Option<Self> {
        let mut current = CONNECTIONS.load(Ordering::Relaxed);
        loop {
            if current >= MAX_CONNECTIONS {
                return None;
            }
            match CONNECTIONS.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Some(Self(())),
                Err(actual) => current = actual,
            }
        }
    }
}

impl Drop for ConnectionSlot {
    fn drop(&mut self) {
        CONNECTIONS.fetch_sub(1, Ordering::AcqRel);
    }
}

pub(crate) async fn upgrade(
    State(state): State<AppState>,
    Extension(authenticated): Extension<AuthenticatedGatewayApiKey>,
    headers: HeaderMap,
    ws: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Response {
    let Ok(ws) = ws else {
        // A plain GET without upgrade headers keeps the pre-WebSocket answer.
        return super::error::method_not_allowed_response(
            &state,
            &axum::http::Uri::from_static("/v1/responses"),
        );
    };
    let Some(slot) = ConnectionSlot::acquire() else {
        return upgrade_required_response();
    };
    let context = WsConnectionContext {
        state,
        gateway_api_key_id: authenticated.id(),
        client_ip: authenticated.client_ip(),
        base_headers: base_headers(headers),
        _slot: slot,
    };
    // tungstenite's default frame cap is far below the message cap; large
    // requests arrive as single unfragmented frames, so both must match.
    ws.max_message_size(bridge::MAX_MESSAGE_BYTES)
        .max_frame_size(bridge::MAX_MESSAGE_BYTES)
        .on_upgrade(move |socket| connection::run(socket, context))
}

/// The upgrade request headers become the per-message base header surface;
/// the WebSocket handshake fields themselves must not reach the pipeline.
fn base_headers(mut headers: HeaderMap) -> HeaderMap {
    for name in [
        "connection",
        "upgrade",
        "keep-alive",
        "sec-websocket-key",
        "sec-websocket-version",
        "sec-websocket-protocol",
        "sec-websocket-extensions",
    ] {
        headers.remove(HeaderName::from_static(name));
    }
    headers
}

fn upgrade_required_response() -> Response {
    let body = serde_json::json!({
        "error": {
            "type": "invalid_request_error",
            "code": "websocket_unavailable",
            "message": "the websocket connection limit was reached; use the HTTP transport",
        }
    })
    .to_string();
    Response::builder()
        .status(StatusCode::UPGRADE_REQUIRED)
        .header("content-type", "application/json")
        .body(axum::body::Body::from(body))
        .expect("static upgrade-required response")
}
