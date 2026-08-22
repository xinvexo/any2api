use std::collections::BTreeMap;

use any2api_domain::TokenUsage;
use serde_json::{Value, json};

use super::{
    super::super::{
        response::{incomplete_reason, responses_usage, token_usage},
        response_projection::ResponseProjection,
        tool_projection::ToolProjection,
    },
    chunk,
    items::TextState,
    tools::ToolState,
};
use crate::api::OpenAiChatCompletionsProfile;
use crate::{
    ProtocolError,
    api::{AdapterEvent, ProtocolEventTelemetry, SseEventPayload, StreamTermination},
};

use super::super::wire::{SynthesizedEvent, encode_event, event, terminal_event};
use super::failure;

pub(crate) struct StreamUpdate {
    pub(crate) events: Vec<AdapterEvent>,
    pub(crate) assistant_message: Option<Value>,
}

pub(crate) struct ChatToResponsesStream {
    pub(super) response: ResponseProjection,
    pub(super) profile: OpenAiChatCompletionsProfile,
    pub(super) projection: ToolProjection,
    pub(super) model: String,
    pub(super) created_at: u64,
    pub(super) observed_model: Option<String>,
    pub(super) observed_created_at: Option<u64>,
    pub(super) observed_role: bool,
    pub(super) started: bool,
    pub(super) completed: bool,
    pub(super) finish_reason: Option<String>,
    pub(super) usage: TokenUsage,
    pub(super) usage_json: Option<Value>,
    next_sequence_number: u64,
    pub(super) next_output_index: usize,
    pub(super) reasoning: TextState,
    pub(super) message: TextState,
    pub(super) tools: BTreeMap<u64, ToolState>,
    pub(super) completed_items: Vec<(usize, Value)>,
}

impl ChatToResponsesStream {
    pub(crate) fn new(
        response: ResponseProjection,
        profile: OpenAiChatCompletionsProfile,
        projection: ToolProjection,
    ) -> Self {
        let model = response.upstream_model().to_owned();
        let created_at = response.created_at();
        Self {
            response,
            profile,
            projection,
            model,
            created_at,
            observed_model: None,
            observed_created_at: None,
            observed_role: false,
            started: false,
            completed: false,
            finish_reason: None,
            usage: TokenUsage::default(),
            usage_json: None,
            next_sequence_number: 0,
            next_output_index: 0,
            reasoning: TextState::default(),
            message: TextState::default(),
            tools: BTreeMap::new(),
            completed_items: Vec::new(),
        }
    }

    pub(crate) fn push(&mut self, event: AdapterEvent) -> Result<StreamUpdate, ProtocolError> {
        if self.completed {
            return Err(invalid(
                "Chat Completions stream continued after termination",
            ));
        }
        self.usage.merge(event.telemetry().token_usage);
        let termination = event.termination();
        let upstream_failure = event.upstream_failure().cloned();
        let (_, payload) = event.into_parts();
        let value = match payload {
            SseEventPayload::Json(data) => data
                .to_value()
                .map_err(|_| invalid("Chat Completions stream data is not valid JSON"))?,
            SseEventPayload::NonJson => {
                return Err(invalid("Chat Completions stream data is not valid JSON"));
            }
            SseEventPayload::Done => return self.finish(),
            SseEventPayload::Empty => {
                return Ok(StreamUpdate {
                    events: Vec::new(),
                    assistant_message: None,
                });
            }
        };
        if let Some(usage) = value.get("usage").filter(|usage| !usage.is_null()) {
            responses_usage(Some(usage), self.profile)?;
            self.usage_json = Some(usage.clone());
            self.usage.merge(token_usage(Some(usage), self.profile)?);
        }
        if termination == StreamTermination::Failed {
            let events = self.sequence_events(vec![failure::convert(&value, upstream_failure)?])?;
            self.completed = true;
            return Ok(StreamUpdate {
                events,
                assistant_message: None,
            });
        }
        chunk::observe_metadata(self, &value)?;
        let mut events = self.ensure_started();
        let choices = match value.get("choices") {
            Some(Value::Array(choices)) => choices,
            None if value.get("usage").is_some_and(Value::is_object) => {
                return Ok(StreamUpdate {
                    events: self.sequence_events(events)?,
                    assistant_message: None,
                });
            }
            _ => {
                return Err(invalid(
                    "Chat Completions stream event has no choices array",
                ));
            }
        };
        chunk::apply_choices(self, choices, &mut events)?;
        if self.finish_reason.is_some() {
            self.validate_tool_identities()?;
        }
        Ok(StreamUpdate {
            events: self.sequence_events(events)?,
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
        self.validate_tool_identities()?;
        let finish_reason = self
            .finish_reason
            .clone()
            .ok_or_else(|| invalid("Chat Completions stream ended without finish_reason"))?;
        if !self.observed_role {
            return Err(invalid(
                "Chat Completions stream never identified the assistant role",
            ));
        }
        if finish_reason == "tool_calls" && self.tools.is_empty() {
            return Err(invalid(
                "finish_reason tool_calls requires at least one tool call",
            ));
        }
        if finish_reason != "tool_calls" && !self.tools.is_empty() {
            return Err(invalid(
                "streamed tool calls require finish_reason tool_calls",
            ));
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
        let incomplete_reason = incomplete_reason(Some(&finish_reason));
        let status = if incomplete_reason.is_some() {
            "incomplete"
        } else {
            "completed"
        };
        let usage = match self.usage_json.as_ref() {
            Some(usage) => responses_usage(Some(usage), self.profile)?,
            None => Value::Null,
        };
        let mut response = self.base_response(status, output, usage);
        if let Some(reason) = incomplete_reason {
            response["incomplete_details"] = json!({"reason":reason});
        }
        let terminal_kind = if status == "incomplete" {
            "response.incomplete"
        } else {
            "response.completed"
        };
        events.push(terminal_event(
            terminal_kind,
            json!({"type":terminal_kind,"response":response}),
            ProtocolEventTelemetry {
                token_usage: self.usage,
                effective_speed_tier: None,
                has_content_delta: false,
                retry_transparent: false,
            },
            StreamTermination::Completed,
        ));
        let events = self.sequence_events(events)?;
        self.completed = true;
        Ok(StreamUpdate {
            events,
            assistant_message: Some(self.assistant_message()),
        })
    }

    fn sequence_events(
        &mut self,
        events: Vec<SynthesizedEvent>,
    ) -> Result<Vec<AdapterEvent>, ProtocolError> {
        events
            .into_iter()
            .map(|mut event| {
                let object = event
                    .data_mut()
                    .as_object_mut()
                    .ok_or_else(|| invalid("bridged Responses event data must be an object"))?;
                object.insert(
                    "sequence_number".to_owned(),
                    json!(self.next_sequence_number),
                );
                self.next_sequence_number = self
                    .next_sequence_number
                    .checked_add(1)
                    .ok_or_else(|| invalid("bridged Responses sequence number overflowed"))?;
                Ok(encode_event(event))
            })
            .collect()
    }

    fn ensure_started(&mut self) -> Vec<SynthesizedEvent> {
        if self.started {
            return Vec::new();
        }
        self.started = true;
        let response = self.base_response("in_progress", Vec::new(), Value::Null);
        vec![
            event(
                "response.created",
                json!({"type":"response.created","response":response}),
                ProtocolEventTelemetry {
                    retry_transparent: true,
                    ..ProtocolEventTelemetry::default()
                },
            ),
            event(
                "response.in_progress",
                json!({"type":"response.in_progress","response":response}),
                ProtocolEventTelemetry {
                    retry_transparent: true,
                    ..ProtocolEventTelemetry::default()
                },
            ),
        ]
    }

    fn base_response(&self, status: &str, output: Vec<Value>, usage: Value) -> Value {
        self.response
            .base_response(status, output, self.created_at, &self.model, usage)
    }

    pub(super) fn response_id(&self) -> &str {
        self.response.response_id()
    }
}

fn invalid(message: &'static str) -> ProtocolError {
    ProtocolError::InvalidPayload(message.into())
}
