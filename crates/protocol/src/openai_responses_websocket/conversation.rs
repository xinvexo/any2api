//! Connection-scoped incremental request reconstruction (ADR-0151).
//!
//! Mirrors the client contract: the full logical input of an incremental
//! request equals the previous request input, followed by the previous
//! response's `response.output_item.done` items, followed by the new items.
//! A baseline exists only after a `response.completed` termination.

use bytes::Bytes;
use serde_json::Value;

use super::ingress::ResponsesWsIngress;
use crate::ProtocolError;

/// In-memory continuation baseline for one WebSocket connection.
pub struct ResponsesWsConversation {
    baseline: Option<Baseline>,
    max_state_bytes: usize,
}

struct Baseline {
    response_id: String,
    input: Vec<Value>,
    output_items: Vec<Value>,
}

/// A `response.create` message restored to a full stateless request body.
pub struct ResolvedWsRequest {
    body: Bytes,
    effective_input: Vec<Value>,
}

impl ResolvedWsRequest {
    #[must_use]
    pub fn body(&self) -> Bytes {
        self.body.clone()
    }
}

/// What the bridged response stream observed for baseline bookkeeping.
#[derive(Default)]
pub struct WsResponseOutcome {
    pub response_id: Option<String>,
    pub output_items: Vec<Value>,
    pub output_bytes: usize,
    pub completed: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub enum WsResolveError {
    /// `previous_response_id` does not match the connection baseline; the
    /// client recovers by retrying the full request.
    PreviousResponseNotFound,
    RequestTooLarge {
        bytes: usize,
        max_bytes: usize,
    },
    Invalid(ProtocolError),
}

impl ResponsesWsConversation {
    #[must_use]
    pub const fn new(max_state_bytes: usize) -> Self {
        Self {
            baseline: None,
            max_state_bytes,
        }
    }

    /// Restores the full request body for one ingress message, resolving an
    /// incremental payload against the connection baseline.
    pub fn resolve(
        &self,
        ingress: &ResponsesWsIngress,
        max_body_bytes: usize,
    ) -> Result<ResolvedWsRequest, WsResolveError> {
        let effective_input = match ingress.previous_response_id() {
            None => ingress.input().to_vec(),
            Some(id) => {
                let Some(baseline) = self
                    .baseline
                    .as_ref()
                    .filter(|baseline| baseline.response_id == id)
                else {
                    return Err(WsResolveError::PreviousResponseNotFound);
                };
                baseline
                    .input
                    .iter()
                    .chain(baseline.output_items.iter())
                    .chain(ingress.input().iter())
                    .cloned()
                    .collect()
            }
        };
        let mut payload = ingress.payload().clone();
        payload.insert("input".to_owned(), Value::Array(effective_input));
        let body = serde_json::to_vec(&payload).map_err(|_| {
            WsResolveError::Invalid(ProtocolError::Internal(
                "failed to encode restored websocket request".into(),
            ))
        })?;
        if body.len() > max_body_bytes {
            return Err(WsResolveError::RequestTooLarge {
                bytes: body.len(),
                max_bytes: max_body_bytes,
            });
        }
        let Some(Value::Array(effective_input)) = payload.remove("input") else {
            unreachable!("input was inserted above");
        };
        Ok(ResolvedWsRequest {
            body: Bytes::from(body),
            effective_input,
        })
    }

    /// Commits or clears the baseline after a bridged response finished.
    /// Only a completed response with a response ID establishes a baseline.
    pub fn finish_exchange(&mut self, resolved: ResolvedWsRequest, outcome: WsResponseOutcome) {
        self.baseline = None;
        if !outcome.completed {
            return;
        }
        let Some(response_id) = outcome.response_id else {
            return;
        };
        if resolved.body.len().saturating_add(outcome.output_bytes) > self.max_state_bytes {
            return;
        }
        self.baseline = Some(Baseline {
            response_id,
            input: resolved.effective_input,
            output_items: outcome.output_items,
        });
    }

    /// Commits a locally answered warmup: the uploaded input becomes the
    /// baseline under a connection-scoped synthetic response ID.
    pub fn finish_warmup(&mut self, resolved: ResolvedWsRequest, response_id: String) {
        self.baseline = None;
        if resolved.body.len() > self.max_state_bytes {
            return;
        }
        self.baseline = Some(Baseline {
            response_id,
            input: resolved.effective_input,
            output_items: Vec::new(),
        });
    }

    /// Drops the baseline; the next incremental request self-heals through
    /// `previous_response_not_found`.
    pub fn clear(&mut self) {
        self.baseline = None;
    }
}
