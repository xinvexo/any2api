use std::{
    collections::BTreeMap,
    time::{SystemTime, UNIX_EPOCH},
};

use any2api_domain::TokenUsage;
use serde_json::{Value, json};

use super::super::super::response::{responses_usage, token_usage};
use super::items::{TextState, ToolState};
use crate::{
    ProtocolError,
    api::{AdapterEvent, ProtocolEventTelemetry, SseEventPayload, StreamTermination},
};

use super::super::wire::{sse, sse_terminal};

pub(crate) struct StreamUpdate {
    pub(crate) events: Vec<AdapterEvent>,
    pub(crate) assistant_message: Option<Value>,
}

pub(crate) struct ChatToResponsesStream {
    pub(super) response_id: String,
    pub(super) model: String,
    pub(super) created_at: u64,
    pub(super) started: bool,
    pub(super) completed: bool,
    pub(super) finish_reason: Option<String>,
    pub(super) usage: TokenUsage,
    pub(super) usage_json: Option<Value>,
    pub(super) next_output_index: usize,
    pub(super) reasoning: TextState,
    pub(super) message: TextState,
    pub(super) tools: BTreeMap<u64, ToolState>,
    pub(super) completed_items: Vec<(usize, Value)>,
}

impl ChatToResponsesStream {
    pub(crate) fn new(response_id: String, model: String) -> Self {
        Self {
            response_id,
            model,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_secs()),
            started: false,
            completed: false,
            finish_reason: None,
            usage: TokenUsage::default(),
            usage_json: None,
            next_output_index: 0,
            reasoning: TextState::default(),
            message: TextState::default(),
            tools: BTreeMap::new(),
            completed_items: Vec::new(),
        }
    }

    pub(crate) fn push(&mut self, event: AdapterEvent) -> Result<StreamUpdate, ProtocolError> {
        self.usage.merge(event.telemetry().token_usage);
        let (raw, payload) = event.into_parts();
        let value = match payload {
            SseEventPayload::Json { data, .. } => data,
            SseEventPayload::NonJson => {
                return Err(invalid("Chat Completions stream data is not valid JSON"));
            }
            SseEventPayload::Empty => {
                if String::from_utf8_lossy(&raw).contains("[DONE]") {
                    return self.finish();
                }
                return Ok(StreamUpdate {
                    events: Vec::new(),
                    assistant_message: None,
                });
            }
        };
        if let Some(usage) = value.get("usage").filter(|usage| !usage.is_null()) {
            self.usage_json = Some(usage.clone());
            self.usage.merge(token_usage(Some(usage)));
        }
        if let Some(model) = value.get("model").and_then(Value::as_str) {
            self.model = model.to_owned();
        }
        let mut events = self.ensure_started();
        let choices = value
            .get("choices")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("Chat Completions stream event has no choices array"))?;
        if choices.len() > 1 {
            return Err(invalid(
                "Chat Completions stream event must contain at most one choice",
            ));
        }
        if let Some(choice) = choices.first() {
            if let Some(delta) = choice.get("delta") {
                if let Some(reasoning) = delta
                    .get("reasoning_content")
                    .or_else(|| delta.get("reasoning"))
                    .and_then(Value::as_str)
                {
                    events.extend(self.push_reasoning(reasoning));
                }
                if let Some(content) = delta.get("content").and_then(Value::as_str) {
                    events.extend(self.push_text(content));
                }
                if let Some(calls) = delta.get("tool_calls").and_then(Value::as_array) {
                    for call in calls {
                        events.extend(self.push_tool(call)?);
                    }
                }
            }
            if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
                self.finish_reason = Some(reason.to_owned());
            }
        }
        Ok(StreamUpdate {
            events,
            assistant_message: None,
        })
    }

    pub(crate) fn finish(&mut self) -> Result<StreamUpdate, ProtocolError> {
        if self.completed {
            return Ok(StreamUpdate {
                events: Vec::new(),
                assistant_message: None,
            });
        }
        let mut events = self.ensure_started();
        events.extend(self.finish_reasoning());
        events.extend(self.finish_message());
        events.extend(self.finish_tools()?);
        self.completed_items.sort_by_key(|(index, _)| *index);
        let output = self
            .completed_items
            .iter()
            .map(|(_, item)| item.clone())
            .collect::<Vec<_>>();
        let status = if self.finish_reason.as_deref() == Some("length") {
            "incomplete"
        } else {
            "completed"
        };
        let usage = self
            .usage_json
            .as_ref()
            .map(|usage| responses_usage(Some(usage)))
            .unwrap_or_else(|| usage_from_tokens(self.usage));
        let mut response = self.base_response(status, output);
        response["usage"] = usage;
        if status == "incomplete" {
            response["incomplete_details"] = json!({"reason":"max_output_tokens"});
        }
        let terminal_kind = if status == "incomplete" {
            "response.incomplete"
        } else {
            "response.completed"
        };
        events.push(sse_terminal(
            terminal_kind,
            json!({"type":terminal_kind,"response":response}),
            ProtocolEventTelemetry {
                token_usage: self.usage,
                has_content_delta: false,
            },
            StreamTermination::Completed,
        ));
        self.completed = true;
        Ok(StreamUpdate {
            events,
            assistant_message: Some(self.assistant_message()),
        })
    }

    fn ensure_started(&mut self) -> Vec<AdapterEvent> {
        if self.started {
            return Vec::new();
        }
        self.started = true;
        let response = self.base_response("in_progress", Vec::new());
        vec![
            sse(
                "response.created",
                json!({"type":"response.created","response":response}),
                ProtocolEventTelemetry::default(),
            ),
            sse(
                "response.in_progress",
                json!({"type":"response.in_progress","response":response}),
                ProtocolEventTelemetry::default(),
            ),
        ]
    }

    fn base_response(&self, status: &str, output: Vec<Value>) -> Value {
        json!({"id":self.response_id,"object":"response","created_at":self.created_at,
            "status":status,"model":self.model,"output":output,"error":Value::Null,
            "incomplete_details":Value::Null,"usage":Value::Null})
    }
}

fn usage_from_tokens(usage: TokenUsage) -> Value {
    let input = usage.input_tokens().unwrap_or(0);
    let output = usage.output_tokens().unwrap_or(0);
    json!({"input_tokens":input,"output_tokens":output,
        "total_tokens":input.saturating_add(output),
        "input_tokens_details":{"cached_tokens":usage.cache_read_tokens().unwrap_or(0)},
        "output_tokens_details":{"reasoning_tokens":0}})
}

fn invalid(message: &'static str) -> ProtocolError {
    ProtocolError::InvalidPayload(message.into())
}
