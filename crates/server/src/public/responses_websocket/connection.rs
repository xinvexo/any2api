//! Per-connection message loop for the Responses WebSocket ingress.

use std::{net::IpAddr, time::Duration};

use any2api_domain::GatewayApiKeyId;
use any2api_runtime::api::responses_websocket::ResponsesWsConversation;
use axum::{
    extract::ws::{CloseFrame, Message, WebSocket},
    http::HeaderMap,
};

use crate::state::AppState;

use super::{ConnectionSlot, bridge};

/// A connection idle between requests is closed after this long; the client
/// transparently reconnects on its next request.
const IDLE_TIMEOUT: Duration = Duration::from_secs(30 * 60);

pub(super) struct WsConnectionContext {
    pub(super) state: AppState,
    pub(super) gateway_api_key_id: GatewayApiKeyId,
    pub(super) client_ip: IpAddr,
    pub(super) base_headers: HeaderMap,
    /// Holds the connection admission slot for the socket lifetime.
    pub(super) _slot: ConnectionSlot,
}

pub(super) async fn run(mut socket: WebSocket, context: WsConnectionContext) {
    let mut conversation = ResponsesWsConversation::new(bridge::MAX_STATE_BYTES);
    loop {
        let received = match tokio::time::timeout(IDLE_TIMEOUT, socket.recv()).await {
            Ok(received) => received,
            Err(_) => {
                let _ = socket
                    .send(Message::Close(Some(CloseFrame {
                        code: axum::extract::ws::close_code::NORMAL,
                        reason: "idle timeout".into(),
                    })))
                    .await;
                return;
            }
        };
        let text = match received {
            None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return,
            Some(Ok(Message::Ping(_) | Message::Pong(_))) => continue,
            Some(Ok(Message::Binary(_))) => {
                if bridge::send_invalid_request(
                    &mut socket,
                    &context.state,
                    "binary websocket messages are not supported",
                )
                .await
                .is_err()
                {
                    return;
                }
                continue;
            }
            Some(Ok(Message::Text(text))) => text,
        };
        let handled =
            bridge::handle_message(&mut socket, &context, &mut conversation, text.as_str()).await;
        match handled {
            bridge::MessageOutcome::Continue => {}
            bridge::MessageOutcome::Close => return,
        }
    }
}
