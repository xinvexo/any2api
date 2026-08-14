//! OpenAI Responses WebSocket ingress protocol (ADR-0151).
//!
//! The gateway accepts the `responses_websockets=2026-02-06` framing on
//! `/v1/responses`: each `response.create` text message is restored into a
//! transport-independent Responses request that replays the existing public
//! request pipeline, and egress SSE frames are re-delivered as self-describing
//! JSON text messages. Incremental requests and warmups are resolved against a
//! connection-scoped conversation baseline.

mod conversation;
mod egress;
mod error_events;
mod ingress;

#[cfg(test)]
mod tests;

pub use conversation::{
    ResolvedWsRequest, ResponsesWsConversation, WsResolveError, WsResponseOutcome,
};
pub use egress::{
    ResponsesWsEgressFrame, ResponsesWsObservation, classify_egress_frame, warmup_completed_event,
    warmup_created_event,
};
pub use error_events::{previous_response_not_found_event, wrapped_error_event};
pub use ingress::ResponsesWsIngress;
