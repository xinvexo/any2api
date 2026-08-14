//! Bridges one `response.create` message through the public request pipeline.

use std::pin::pin;

use any2api_domain::{ProtocolDialect, ProtocolOperation, PublicError, PublicErrorCode, RequestId};
use any2api_runtime::api::{
    PublicRequest, PublicResponseBody, PublicResponseStream,
    STANDARD_PUBLIC_REQUEST_BODY_LIMIT_BYTES, SseDecoder, SseFrame, request_body_limit,
    responses_websocket::{
        ResolvedWsRequest, ResponsesWsConversation, ResponsesWsIngress, ResponsesWsObservation,
        WsResolveError, WsResponseOutcome, classify_egress_frame,
        previous_response_not_found_event, warmup_completed_event, warmup_created_event,
        wrapped_error_event,
    },
};
use axum::{
    extract::ws::{CloseFrame, Message, Utf8Bytes, WebSocket, close_code},
    http::{HeaderMap, HeaderValue, StatusCode, header::CONTENT_TYPE},
};
use futures_util::StreamExt;

use crate::state::AppState;

use super::connection::WsConnectionContext;

/// Inbound messages and restored request bodies share the standard public
/// request body limit.
pub(super) const MAX_MESSAGE_BYTES: usize = STANDARD_PUBLIC_REQUEST_BODY_LIMIT_BYTES;
pub(super) const MAX_STATE_BYTES: usize = STANDARD_PUBLIC_REQUEST_BODY_LIMIT_BYTES;

/// Re-framing limit for egress SSE produced by the runtime. The runtime has
/// already enforced its own per-frame budgets (up to 64 MiB for remote
/// compaction), so this is only a defensive ceiling above them.
const EGRESS_FRAME_LIMIT_BYTES: usize = 128 * 1024 * 1024;

pub(super) enum MessageOutcome {
    Continue,
    Close,
}

pub(super) async fn handle_message(
    socket: &mut WebSocket,
    context: &WsConnectionContext,
    conversation: &mut ResponsesWsConversation,
    text: &str,
) -> MessageOutcome {
    let state = &context.state;
    let Some(_lifecycle_guard) = state.runtime().lifecycle().track_request() else {
        let event = wrapped_error_event(
            StatusCode::SERVICE_UNAVAILABLE,
            &HeaderMap::new(),
            br#"{"error":{"message":"the server is shutting down"}}"#,
        );
        let _ = send_text(socket, event).await;
        return MessageOutcome::Close;
    };
    let ingress = match ResponsesWsIngress::parse(text) {
        Ok(ingress) => ingress,
        Err(error) => {
            return continue_if_sent(
                send_local_error(
                    socket,
                    state,
                    PublicErrorCode::InvalidRequest,
                    &error.to_string(),
                )
                .await,
            );
        }
    };
    let snapshot = state.snapshots().load();
    let key_active = snapshot
        .gateway_api_keys()
        .keys()
        .iter()
        .any(|key| key.id() == context.gateway_api_key_id && key.is_active());
    if !key_active {
        let _ = send_local_error(
            socket,
            state,
            PublicErrorCode::Unauthorized,
            "a valid Gateway API Key is required",
        )
        .await;
        return MessageOutcome::Close;
    }
    state
        .request_telemetry()
        .record_gateway_key_use(context.gateway_api_key_id, snapshot.revision());

    let resolved = match conversation
        .resolve(&ingress, request_body_limit(ProtocolOperation::Responses))
    {
        Ok(resolved) => resolved,
        Err(WsResolveError::PreviousResponseNotFound) => {
            return continue_if_sent(send_text(socket, previous_response_not_found_event()).await);
        }
        Err(WsResolveError::RequestTooLarge { .. }) => {
            return continue_if_sent(
                send_local_error(
                    socket,
                    state,
                    PublicErrorCode::PayloadTooLarge,
                    "request body exceeds the public API request size limit",
                )
                .await,
            );
        }
        Err(WsResolveError::Invalid(error)) => {
            return continue_if_sent(
                send_local_error(
                    socket,
                    state,
                    PublicErrorCode::InternalError,
                    &error.to_string(),
                )
                .await,
            );
        }
    };

    if ingress.is_warmup() {
        let response_id = format!("resp_any2api_{}", RequestId::new().as_uuid().simple());
        if send_text(socket, warmup_created_event(&response_id))
            .await
            .is_err()
            || send_text(socket, warmup_completed_event(&response_id))
                .await
                .is_err()
        {
            return MessageOutcome::Close;
        }
        conversation.finish_warmup(resolved, response_id);
        return MessageOutcome::Continue;
    }

    let request = PublicRequest {
        request_id: RequestId::new(),
        gateway_api_key_id: context.gateway_api_key_id,
        client_ip: context.client_ip,
        operation: ProtocolOperation::Responses,
        headers: message_headers(&context.base_headers, &ingress),
        body: resolved.body(),
    };
    let mut execute = pin!(state.public_requests().execute(snapshot, request));
    let response = loop {
        tokio::select! {
            biased;
            received = socket.recv() => match received {
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                // Dropping the execute future cancels the in-flight request;
                // runtime guards settle exactly once on drop.
                _ => return MessageOutcome::Close,
            },
            response = &mut execute => break response,
        }
    };

    match response.body {
        PublicResponseBody::Buffered(body) => {
            conversation.clear();
            if response.status.is_success() {
                let _ = send_local_error(
                    socket,
                    state,
                    PublicErrorCode::InternalError,
                    "streaming request produced a buffered response",
                )
                .await;
                return MessageOutcome::Close;
            }
            let event = wrapped_error_event(response.status, &response.headers, &body);
            continue_if_sent(send_text(socket, event).await)
        }
        PublicResponseBody::Streaming(stream) => {
            pump_stream(socket, conversation, resolved, stream).await
        }
    }
}

/// Forwards egress SSE frames as WebSocket text messages while accumulating
/// the next incremental baseline.
async fn pump_stream(
    socket: &mut WebSocket,
    conversation: &mut ResponsesWsConversation,
    resolved: ResolvedWsRequest,
    stream: PublicResponseStream,
) -> MessageOutcome {
    let mut stream = stream;
    let mut decoder = SseDecoder::new(EGRESS_FRAME_LIMIT_BYTES);
    let mut outcome = WsResponseOutcome::default();
    loop {
        let chunk = tokio::select! {
            biased;
            received = socket.recv() => match received {
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => continue,
                // Client went away mid-stream: dropping the body cancels the
                // upstream request through the existing guarded lifecycle.
                _ => return MessageOutcome::Close,
            },
            chunk = stream.next() => chunk,
        };
        match chunk {
            Some(Ok(bytes)) => {
                decoder.push(&bytes);
                loop {
                    match decoder.next_frame() {
                        Ok(Some(frame)) => {
                            if forward_frame(socket, &frame, &mut outcome).await.is_err() {
                                return MessageOutcome::Close;
                            }
                        }
                        Ok(None) => break,
                        Err(_) => return close_truncated(socket).await,
                    }
                }
            }
            Some(Err(_)) => {
                // The stream failed after bytes were committed downstream; a
                // fabricated terminal event would be dishonest, so the whole
                // connection ends like a truncated HTTP response body.
                conversation.clear();
                return close_truncated(socket).await;
            }
            None => {
                if let Ok(Some(frame)) = decoder.finish()
                    && forward_frame(socket, &frame, &mut outcome).await.is_err()
                {
                    return MessageOutcome::Close;
                }
                conversation.finish_exchange(resolved, outcome);
                return MessageOutcome::Continue;
            }
        }
    }
}

async fn forward_frame(
    socket: &mut WebSocket,
    frame: &SseFrame,
    outcome: &mut WsResponseOutcome,
) -> Result<(), ()> {
    let classified = classify_egress_frame(frame);
    match classified.observation {
        ResponsesWsObservation::None => {}
        ResponsesWsObservation::OutputItemDone { item, raw_len } => {
            outcome.output_items.push(item);
            outcome.output_bytes = outcome.output_bytes.saturating_add(raw_len);
        }
        ResponsesWsObservation::Completed { response_id } => {
            outcome.completed = true;
            outcome.response_id = response_id;
        }
    }
    let Some(payload) = classified.payload else {
        return Ok(());
    };
    let Ok(text) = Utf8Bytes::try_from(payload) else {
        return Err(());
    };
    socket.send(Message::Text(text)).await.map_err(|_| ())
}

async fn close_truncated(socket: &mut WebSocket) -> MessageOutcome {
    let _ = socket
        .send(Message::Close(Some(CloseFrame {
            code: close_code::ERROR,
            reason: "upstream stream ended before a terminal event".into(),
        })))
        .await;
    MessageOutcome::Close
}

pub(super) async fn send_invalid_request(
    socket: &mut WebSocket,
    state: &AppState,
    message: &str,
) -> Result<(), ()> {
    send_local_error(socket, state, PublicErrorCode::InvalidRequest, message).await
}

/// Encodes a local error through the Responses adapter so WebSocket errors
/// carry the same OpenAI error object as the HTTP entry, then wraps it in the
/// protocol error event.
async fn send_local_error(
    socket: &mut WebSocket,
    state: &AppState,
    code: PublicErrorCode,
    message: &str,
) -> Result<(), ()> {
    let response = state.public_requests().error_response(
        ProtocolDialect::OpenAiResponses,
        &PublicError::new(code, message),
    );
    let event = match response.body {
        PublicResponseBody::Buffered(body) => {
            wrapped_error_event(response.status, &response.headers, &body)
        }
        PublicResponseBody::Streaming(_) => {
            wrapped_error_event(response.status, &response.headers, b"{}")
        }
    };
    send_text(socket, event).await
}

async fn send_text(socket: &mut WebSocket, text: String) -> Result<(), ()> {
    socket
        .send(Message::Text(Utf8Bytes::from(text)))
        .await
        .map_err(|_| ())
}

const fn continue_if_sent(sent: Result<(), ()>) -> MessageOutcome {
    match sent {
        Ok(()) => MessageOutcome::Continue,
        Err(()) => MessageOutcome::Close,
    }
}

/// The per-message header surface: the upgrade headers plus the session and
/// turn-state values the HTTP entry would have received as request headers.
/// Session identifiers stay connection-stable when a message omits them;
/// turn-state is bound to a single turn and never replayed from the upgrade.
fn message_headers(base: &HeaderMap, ingress: &ResponsesWsIngress) -> HeaderMap {
    let mut headers = base.clone();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    replace_when_present(&mut headers, "session-id", ingress.session_id());
    replace_when_present(&mut headers, "thread-id", ingress.thread_id());
    headers.remove("x-codex-turn-state");
    replace_when_present(&mut headers, "x-codex-turn-state", ingress.turn_state());
    headers
}

fn replace_when_present(headers: &mut HeaderMap, name: &'static str, value: Option<&str>) {
    if let Some(value) = value
        && let Ok(value) = HeaderValue::from_str(value)
    {
        headers.insert(name, value);
    }
}
