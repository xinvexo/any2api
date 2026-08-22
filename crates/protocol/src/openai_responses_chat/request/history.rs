mod output;

use std::collections::HashMap;

use serde_json::{Map, Value, json};

use crate::{
    ProtocolError,
    api::OpenAiChatCompletionsProfile,
    openai_responses_chat::tool_projection::{ProjectedSourceCall, SourceCallKind, ToolProjection},
};

use super::{
    input::{convert_content, reasoning_text, text_value},
    invalid,
    ledger::ConversationLedger,
    required_string,
};

pub(super) fn append_items(
    items: &[Value],
    messages: &mut Vec<Value>,
    projection: &mut ToolProjection,
    profile: OpenAiChatCompletionsProfile,
) -> Result<(), ProtocolError> {
    let outputs = collect_output_call_ids(items)?;
    let ledger = ConversationLedger::from_messages(messages, projection)?;
    let mut assembler = InputAssembler::new(messages, outputs, ledger, projection, profile);
    for item in items {
        assembler.append(item)?;
    }
    assembler.finish()
}

fn collect_output_call_ids(
    items: &[Value],
) -> Result<HashMap<String, SourceCallKind>, ProtocolError> {
    let mut ids = HashMap::new();
    for item in items {
        let Some(object) = item.as_object() else {
            continue;
        };
        let kind = match object.get("type").and_then(Value::as_str) {
            Some("function_call_output") => SourceCallKind::Function,
            Some("custom_tool_call_output") => SourceCallKind::Custom,
            Some("tool_search_output") => SourceCallKind::ToolSearch,
            _ => continue,
        };
        let call_id = required_string(object.get("call_id"), "tool call output.call_id")?;
        if ids.insert(call_id.to_owned(), kind).is_some() {
            return Err(invalid(
                "tool call output.call_id must be unique in the current input",
            ));
        }
    }
    Ok(ids)
}

struct InputAssembler<'a> {
    messages: &'a mut Vec<Value>,
    projection: &'a mut ToolProjection,
    profile: OpenAiChatCompletionsProfile,
    pending_calls: Vec<Value>,
    pending_reasoning: String,
    output_call_ids: HashMap<String, SourceCallKind>,
    ledger: ConversationLedger,
    deferred_messages: Vec<Value>,
}

impl<'a> InputAssembler<'a> {
    fn new(
        messages: &'a mut Vec<Value>,
        output_call_ids: HashMap<String, SourceCallKind>,
        ledger: ConversationLedger,
        projection: &'a mut ToolProjection,
        profile: OpenAiChatCompletionsProfile,
    ) -> Self {
        Self {
            messages,
            projection,
            profile,
            pending_calls: Vec::new(),
            pending_reasoning: String::new(),
            output_call_ids,
            ledger,
            deferred_messages: Vec::new(),
        }
    }

    fn append(&mut self, item: &Value) -> Result<(), ProtocolError> {
        let object = item
            .as_object()
            .ok_or_else(|| invalid("input items must be objects"))?;
        let kind = object.get("type").and_then(Value::as_str);
        if !matches!(
            kind,
            Some("function_call" | "custom_tool_call" | "tool_search_call")
        ) {
            self.flush_calls()?;
        }
        match kind {
            Some("message") | None if object.contains_key("role") => self.append_message(object),
            Some("input_text") => {
                self.flush_reasoning()?;
                let text = required_string(object.get("text"), "input_text.text")?;
                self.append_regular(json!({"role":"user","content":text}));
                Ok(())
            }
            Some("function_call") => self.append_call(object, SourceCallKind::Function),
            Some("custom_tool_call") => self.append_call(object, SourceCallKind::Custom),
            Some("tool_search_call") => self.append_call(object, SourceCallKind::ToolSearch),
            Some("function_call_output") => self.append_output(object, SourceCallKind::Function),
            Some("custom_tool_call_output") => self.append_output(object, SourceCallKind::Custom),
            Some("tool_search_output") => self.append_output(object, SourceCallKind::ToolSearch),
            Some("reasoning") => {
                self.pending_reasoning.push_str(&reasoning_text(item));
                Ok(())
            }
            Some(kind) => Err(ProtocolError::unsupported_value("input item type", kind)),
            None => Err(invalid("Responses input item type is required")),
        }
    }

    fn append_message(&mut self, object: &Map<String, Value>) -> Result<(), ProtocolError> {
        let role = match object.get("role").and_then(Value::as_str) {
            Some("developer" | "system") => self.profile.instruction_role.as_str(),
            Some("user") => "user",
            Some("assistant") => "assistant",
            Some(role) => return Err(ProtocolError::unsupported_value("message role", role)),
            None => return Err(invalid("message role is required")),
        };
        if role != "assistant" {
            self.flush_reasoning()?;
        }
        let content = convert_content(
            object
                .get("content")
                .ok_or_else(|| invalid("message content is required"))?,
            role,
            self.profile,
        )?;
        let mut message = json!({"role":role,"content":content});
        if role == "assistant" && !self.pending_reasoning.is_empty() {
            output::insert_reasoning(
                &mut message,
                std::mem::take(&mut self.pending_reasoning),
                self.profile,
            )?;
        }
        self.append_regular(message);
        Ok(())
    }

    fn append_call(
        &mut self,
        object: &Map<String, Value>,
        kind: SourceCallKind,
    ) -> Result<(), ProtocolError> {
        let call_id = required_string(object.get("call_id"), "tool call.call_id")?;
        if self.output_call_ids.get(call_id) != Some(&kind) {
            let message = match kind {
                SourceCallKind::Function => {
                    "current function_call has no unique matching function_call_output"
                }
                SourceCallKind::Custom => {
                    "current custom_tool_call has no unique matching custom_tool_call_output"
                }
                SourceCallKind::ToolSearch => {
                    "current tool_search_call has no unique matching tool_search_output"
                }
            };
            return Err(invalid(message));
        }
        let ProjectedSourceCall {
            mut chat_call,
            identity,
        } = self.projection.project_source_call(object, kind)?;
        self.ledger.add_call(call_id, identity)?;
        chat_call["id"] = Value::String(call_id.to_owned());
        self.pending_calls.push(chat_call);
        Ok(())
    }

    fn append_output(
        &mut self,
        object: &Map<String, Value>,
        kind: SourceCallKind,
    ) -> Result<(), ProtocolError> {
        let call_id = required_string(object.get("call_id"), "tool call output.call_id")?;
        let identity = self.ledger.settle(call_id, kind)?;
        output::validate_identity(object, &identity)?;
        let content = match kind {
            SourceCallKind::Function | SourceCallKind::Custom => {
                let output = object
                    .get("output")
                    .ok_or_else(|| invalid("tool call output.output is required"))?;
                text_value(output)?
            }
            SourceCallKind::ToolSearch => {
                let tools = output::tool_search_tools(object)?;
                self.projection.activate_loaded_tools(tools)?;
                output::tool_search_output(object, tools)?
            }
        };
        self.messages.push(json!({
            "role":"tool","tool_call_id":call_id,"content":content
        }));
        if self.ledger.is_settled() {
            self.flush_deferred();
        }
        Ok(())
    }

    fn append_regular(&mut self, message: Value) {
        if self.ledger.is_settled() {
            self.messages.push(message);
        } else {
            self.deferred_messages.push(message);
        }
    }

    fn flush_calls(&mut self) -> Result<(), ProtocolError> {
        if self.pending_calls.is_empty() {
            return Ok(());
        }
        let mut message = json!({
            "role":"assistant","content":Value::Null,
            "tool_calls":std::mem::take(&mut self.pending_calls)
        });
        if !self.pending_reasoning.is_empty() {
            output::insert_reasoning(
                &mut message,
                std::mem::take(&mut self.pending_reasoning),
                self.profile,
            )?;
        }
        self.messages.push(message);
        Ok(())
    }

    fn flush_reasoning(&mut self) -> Result<(), ProtocolError> {
        if self.pending_reasoning.is_empty() {
            return Ok(());
        }
        let mut message = json!({"role":"assistant","content":Value::Null});
        output::insert_reasoning(
            &mut message,
            std::mem::take(&mut self.pending_reasoning),
            self.profile,
        )?;
        self.append_regular(message);
        Ok(())
    }

    fn flush_deferred(&mut self) {
        self.messages.append(&mut self.deferred_messages);
    }

    fn finish(mut self) -> Result<(), ProtocolError> {
        self.flush_calls()?;
        self.flush_reasoning()?;
        if !self.ledger.is_settled() {
            return Err(invalid(
                "tool call has no matching output before the next model turn",
            ));
        }
        self.flush_deferred();
        Ok(())
    }
}
