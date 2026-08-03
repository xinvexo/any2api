use std::collections::HashSet;

use serde_json::{Map, Value, json};

use crate::ProtocolError;

use super::{
    input::{convert_content, reasoning_text, text_value},
    invalid, required_string,
};

pub(super) fn append_items(
    items: &[Value],
    messages: &mut Vec<Value>,
) -> Result<(), ProtocolError> {
    let output_call_ids = collect_output_call_ids(items)?;
    let mut assembler = InputAssembler::new(messages, output_call_ids);
    for item in items {
        assembler.append(item)?;
    }
    assembler.finish();
    Ok(())
}

fn collect_output_call_ids(items: &[Value]) -> Result<HashSet<String>, ProtocolError> {
    let mut ids = HashSet::new();
    for item in items {
        let Some(object) = item.as_object() else {
            continue;
        };
        if object.get("type").and_then(Value::as_str) != Some("function_call_output") {
            continue;
        }
        let call_id = required_string(object.get("call_id"), "function_call_output.call_id")?;
        if !ids.insert(call_id.to_owned()) {
            return Err(invalid("function_call_output.call_id must be unique"));
        }
    }
    Ok(ids)
}

struct InputAssembler<'a> {
    messages: &'a mut Vec<Value>,
    pending_calls: Vec<Value>,
    pending_call_ids: Vec<String>,
    pending_reasoning: String,
    output_call_ids: HashSet<String>,
    awaiting_outputs: HashSet<String>,
    seen_call_ids: HashSet<String>,
    deferred_messages: Vec<Value>,
}

impl<'a> InputAssembler<'a> {
    fn new(messages: &'a mut Vec<Value>, output_call_ids: HashSet<String>) -> Self {
        Self {
            messages,
            pending_calls: Vec::new(),
            pending_call_ids: Vec::new(),
            pending_reasoning: String::new(),
            output_call_ids,
            awaiting_outputs: HashSet::new(),
            seen_call_ids: HashSet::new(),
            deferred_messages: Vec::new(),
        }
    }

    fn append(&mut self, item: &Value) -> Result<(), ProtocolError> {
        let object = item
            .as_object()
            .ok_or_else(|| invalid("input items must be objects"))?;
        let kind = object.get("type").and_then(Value::as_str);
        if kind != Some("function_call") {
            self.flush_calls();
        }
        match kind {
            Some("message") | None if object.contains_key("role") => self.append_message(object),
            Some("input_text") => {
                self.flush_reasoning();
                let text = required_string(object.get("text"), "input_text.text")?;
                self.append_regular(json!({"role":"user","content":text}));
                Ok(())
            }
            Some("function_call") => self.append_call(object),
            Some("function_call_output") => self.append_output(object),
            Some("reasoning") => {
                self.pending_reasoning.push_str(&reasoning_text(item));
                Ok(())
            }
            Some(_) => Err(invalid(
                "Responses input item is not supported by this bridge",
            )),
            None => Err(invalid("Responses input item type is required")),
        }
    }

    fn append_message(&mut self, object: &Map<String, Value>) -> Result<(), ProtocolError> {
        let role = match object.get("role").and_then(Value::as_str) {
            Some("developer" | "system") => "system",
            Some("user") => "user",
            Some("assistant") => "assistant",
            _ => return Err(invalid("message role is not supported by Chat Completions")),
        };
        if role != "assistant" {
            self.flush_reasoning();
        }
        let content = convert_content(
            object
                .get("content")
                .ok_or_else(|| invalid("message content is required"))?,
            role,
        )?;
        let mut message = json!({"role":role,"content":content});
        if role == "assistant" && !self.pending_reasoning.is_empty() {
            message["reasoning_content"] =
                Value::String(std::mem::take(&mut self.pending_reasoning));
        }
        self.append_regular(message);
        Ok(())
    }

    fn append_call(&mut self, object: &Map<String, Value>) -> Result<(), ProtocolError> {
        let call_id = required_string(object.get("call_id"), "function_call.call_id")?;
        if !self.seen_call_ids.insert(call_id.to_owned()) {
            return Err(invalid("function_call.call_id must be unique"));
        }
        let name = required_string(object.get("name"), "function_call.name")?;
        let arguments = required_string(object.get("arguments"), "function_call.arguments")?;
        if !self.output_call_ids.contains(call_id) {
            return Err(invalid(
                "function_call has no matching function_call_output",
            ));
        }
        self.pending_calls.push(json!({
            "id":call_id,
            "type":"function",
            "function":{"name":name,"arguments":arguments}
        }));
        self.pending_call_ids.push(call_id.to_owned());
        Ok(())
    }

    fn append_output(&mut self, object: &Map<String, Value>) -> Result<(), ProtocolError> {
        let call_id = required_string(object.get("call_id"), "function_call_output.call_id")?;
        let output = object
            .get("output")
            .ok_or_else(|| invalid("function_call_output.output is required"))?;
        self.messages.push(json!({
            "role":"tool",
            "tool_call_id":call_id,
            "content":text_value(output)?
        }));
        self.awaiting_outputs.remove(call_id);
        if self.awaiting_outputs.is_empty() {
            self.flush_deferred();
        }
        Ok(())
    }

    fn append_regular(&mut self, message: Value) {
        if self
            .awaiting_outputs
            .iter()
            .any(|call_id| self.output_call_ids.contains(call_id))
        {
            self.deferred_messages.push(message);
        } else {
            self.messages.push(message);
        }
    }

    fn flush_calls(&mut self) {
        if self.pending_calls.is_empty() {
            return;
        }
        let mut message = json!({
            "role":"assistant",
            "content":Value::Null,
            "tool_calls":std::mem::take(&mut self.pending_calls)
        });
        if !self.pending_reasoning.is_empty() {
            message["reasoning_content"] =
                Value::String(std::mem::take(&mut self.pending_reasoning));
        }
        self.messages.push(message);
        for call_id in self.pending_call_ids.drain(..) {
            self.awaiting_outputs.insert(call_id);
        }
    }

    fn flush_reasoning(&mut self) {
        if self.pending_reasoning.is_empty() {
            return;
        }
        let reasoning = std::mem::take(&mut self.pending_reasoning);
        self.append_regular(
            json!({"role":"assistant","reasoning_content":reasoning,"content":Value::Null}),
        );
    }

    fn flush_deferred(&mut self) {
        self.messages.append(&mut self.deferred_messages);
    }

    fn finish(mut self) {
        self.flush_calls();
        self.flush_reasoning();
        self.flush_deferred();
    }
}
