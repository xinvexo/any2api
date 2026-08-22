use std::collections::{BTreeMap, HashSet};

use serde_json::Value;

use crate::{
    ProtocolError,
    openai_responses_chat::tool_projection::{
        ChatToolKind, SourceCallKind, ToolIdentity, ToolProjection,
    },
};

pub(super) struct ConversationLedger {
    seen_calls: HashSet<String>,
    outstanding: BTreeMap<String, ToolIdentity>,
}

impl ConversationLedger {
    pub(super) fn from_messages(
        messages: &[Value],
        projection: &ToolProjection,
    ) -> Result<Self, ProtocolError> {
        let mut ledger = Self {
            seen_calls: HashSet::new(),
            outstanding: BTreeMap::new(),
        };
        for message in messages {
            let object = message
                .as_object()
                .ok_or_else(|| invalid("conversation message must be an object"))?;
            match object.get("role").and_then(Value::as_str) {
                Some("assistant") => {
                    if !ledger.outstanding.is_empty() {
                        return Err(invalid(
                            "assistant message appears before outstanding tool calls are settled",
                        ));
                    }
                    if let Some(calls) = object.get("tool_calls") {
                        ledger.scan_assistant_calls(calls, projection)?;
                    }
                }
                Some("tool") => ledger.scan_tool_output(object)?,
                Some(_) if ledger.outstanding.is_empty() => {}
                Some(_) => {
                    return Err(invalid(
                        "regular message appears before outstanding tool calls are settled",
                    ));
                }
                None => return Err(invalid("conversation message role is required")),
            }
        }
        Ok(ledger)
    }

    pub(super) fn add_call(
        &mut self,
        call_id: &str,
        identity: ToolIdentity,
    ) -> Result<(), ProtocolError> {
        if !self.seen_calls.insert(call_id.to_owned()) {
            return Err(invalid(
                "tool call.call_id must be unique across the conversation",
            ));
        }
        self.outstanding.insert(call_id.to_owned(), identity);
        Ok(())
    }

    pub(super) fn settle(
        &mut self,
        call_id: &str,
        output_kind: SourceCallKind,
    ) -> Result<ToolIdentity, ProtocolError> {
        let Some(identity) = self.outstanding.remove(call_id) else {
            let message = if self.seen_calls.contains(call_id) {
                "tool call output settles an already completed call"
            } else {
                "tool call output references an unknown call_id"
            };
            return Err(invalid(message));
        };
        if identity.source_call_kind() != output_kind {
            self.outstanding.insert(call_id.to_owned(), identity);
            return Err(invalid("tool call output type does not match its call"));
        }
        Ok(identity)
    }

    pub(super) fn is_settled(&self) -> bool {
        self.outstanding.is_empty()
    }

    fn scan_assistant_calls(
        &mut self,
        value: &Value,
        projection: &ToolProjection,
    ) -> Result<(), ProtocolError> {
        let calls = value
            .as_array()
            .ok_or_else(|| invalid("assistant tool_calls must be an array"))?;
        for call in calls {
            let object = call
                .as_object()
                .ok_or_else(|| invalid("assistant tool call must be an object"))?;
            let call_id = required_string(object.get("id"), "assistant tool call id")?;
            let (kind, payload) = match object.get("type").and_then(Value::as_str) {
                Some("function") => (ChatToolKind::Function, object.get("function")),
                Some("custom") => (ChatToolKind::Custom, object.get("custom")),
                _ => return Err(invalid("assistant tool call type is unsupported")),
            };
            let name = required_string(
                payload.and_then(|value| value.get("name")),
                "assistant tool call name",
            )?;
            self.add_call(call_id, projection.known_identity(kind, name)?)?;
        }
        Ok(())
    }

    fn scan_tool_output(
        &mut self,
        message: &serde_json::Map<String, Value>,
    ) -> Result<(), ProtocolError> {
        let call_id = required_string(message.get("tool_call_id"), "tool message tool_call_id")?;
        let Some(identity) = self.outstanding.remove(call_id) else {
            return Err(invalid(
                "conversation tool output references an unknown call_id",
            ));
        };
        drop(identity);
        Ok(())
    }
}

fn required_string<'a>(
    value: Option<&'a Value>,
    field: &'static str,
) -> Result<&'a str, ProtocolError> {
    value
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid(field))
}

fn invalid(message: &'static str) -> ProtocolError {
    ProtocolError::InvalidPayload(message.into())
}
