mod capability;
mod continuation;
mod contracts;
mod exchange;
mod execution_profile;
mod response;

/// OpenAI Responses WebSocket ingress framing (ADR-0151).
pub mod responses_websocket {
    pub use crate::openai_responses_websocket::{
        ResolvedWsRequest, ResponsesWsConversation, ResponsesWsEgressFrame, ResponsesWsIngress,
        ResponsesWsObservation, WsResolveError, WsResponseOutcome, classify_egress_frame,
        previous_response_not_found_event, warmup_completed_event, warmup_created_event,
        wrapped_error_event,
    };
}

pub use crate::sse::SseDecoder;
pub use capability::*;
pub(crate) use continuation::ResumableProtocolContinuation;
pub use continuation::{
    BridgeContinuationState, MAX_BRIDGE_CONTINUATION_STATE_BYTES, ProtocolContinuationState,
};
pub use contracts::*;
pub use exchange::{PreparedProtocolRequest, ProtocolExchange, StartedProtocolBridge};
pub use execution_profile::RequestExecutionProfile;
pub use response::*;
