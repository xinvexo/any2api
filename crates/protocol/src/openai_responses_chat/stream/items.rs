use serde_json::{Value, json};

use super::{
    ChatToResponsesStream,
    wire::{content_telemetry, sse, sse_default},
};
use crate::{
    ProtocolError,
    api::AdapterEvent,
    openai_responses_chat::response::{function_call_item, message_item, reasoning_item},
};

#[derive(Default)]
pub(super) struct TextState {
    output_index: Option<usize>,
    item_id: String,
    text: String,
    done: bool,
}

#[derive(Default)]
pub(super) struct ToolState {
    output_index: Option<usize>,
    item_id: String,
    call_id: String,
    name: String,
    arguments: String,
    emitted_arguments: usize,
    done: bool,
}

impl ChatToResponsesStream {
    pub(super) fn push_text(&mut self, delta: &str) -> Vec<AdapterEvent> {
        if delta.is_empty() {
            return Vec::new();
        }
        let mut events = Vec::new();
        if self.message.output_index.is_none() {
            let index = self.allocate_output();
            self.message.output_index = Some(index);
            self.message.item_id = format!("{}_msg", self.response_id);
            events.push(sse_default(
                "response.output_item.added",
                json!({"type":"response.output_item.added","output_index":index,"item":{
                    "id":self.message.item_id,"type":"message","status":"in_progress",
                    "role":"assistant","content":[]
                }}),
            ));
            events.push(sse_default(
                "response.content_part.added",
                json!({"type":"response.content_part.added","item_id":self.message.item_id,
                    "output_index":index,"content_index":0,
                    "part":{"type":"output_text","text":"","annotations":[]}}),
            ));
        }
        self.message.text.push_str(delta);
        events.push(sse(
            "response.output_text.delta",
            json!({"type":"response.output_text.delta","item_id":self.message.item_id,
                "output_index":self.message.output_index.unwrap_or(0),"content_index":0,
                "delta":delta}),
            content_telemetry(),
        ));
        events
    }

    pub(super) fn push_reasoning(&mut self, delta: &str) -> Vec<AdapterEvent> {
        if delta.is_empty() {
            return Vec::new();
        }
        let mut events = Vec::new();
        if self.reasoning.output_index.is_none() {
            let index = self.allocate_output();
            self.reasoning.output_index = Some(index);
            self.reasoning.item_id = format!("rs_{}", self.response_id);
            events.push(sse_default(
                "response.output_item.added",
                json!({"type":"response.output_item.added","output_index":index,"item":{
                    "id":self.reasoning.item_id,"type":"reasoning","status":"in_progress","summary":[]
                }}),
            ));
            events.push(sse_default(
                "response.reasoning_summary_part.added",
                json!({"type":"response.reasoning_summary_part.added",
                    "item_id":self.reasoning.item_id,"output_index":index,"summary_index":0,
                    "part":{"type":"summary_text","text":""}}),
            ));
        }
        self.reasoning.text.push_str(delta);
        events.push(sse(
            "response.reasoning_summary_text.delta",
            json!({"type":"response.reasoning_summary_text.delta",
                "item_id":self.reasoning.item_id,
                "output_index":self.reasoning.output_index.unwrap_or(0),
                "summary_index":0,"delta":delta}),
            content_telemetry(),
        ));
        events
    }

    pub(super) fn push_tool(&mut self, call: &Value) -> Result<Vec<AdapterEvent>, ProtocolError> {
        let key = call.get("index").and_then(Value::as_u64).unwrap_or(0);
        let state = self.tools.entry(key).or_default();
        append_fragment(&mut state.call_id, call.get("id"));
        let function = call.get("function").unwrap_or(&Value::Null);
        append_fragment(&mut state.name, function.get("name"));
        append_fragment(&mut state.arguments, function.get("arguments"));
        let mut events = Vec::new();
        if state.output_index.is_none() && !state.call_id.is_empty() && !state.name.is_empty() {
            let index = self.next_output_index;
            self.next_output_index += 1;
            state.output_index = Some(index);
            state.item_id = format!("{}_fc_{key}", self.response_id);
            events.push(sse_default(
                "response.output_item.added",
                json!({"type":"response.output_item.added","output_index":index,"item":{
                    "id":state.item_id,"type":"function_call","status":"in_progress",
                    "call_id":state.call_id,"name":state.name,"arguments":""
                }}),
            ));
        }
        if let Some(index) = state.output_index
            && state.arguments.len() > state.emitted_arguments
        {
            let delta = state.arguments[state.emitted_arguments..].to_owned();
            state.emitted_arguments = state.arguments.len();
            events.push(sse(
                "response.function_call_arguments.delta",
                json!({"type":"response.function_call_arguments.delta","item_id":state.item_id,
                    "output_index":index,"delta":delta}),
                content_telemetry(),
            ));
        }
        Ok(events)
    }

    pub(super) fn finish_message(&mut self) -> Vec<AdapterEvent> {
        let Some(index) = self.message.output_index else {
            return Vec::new();
        };
        if self.message.done {
            return Vec::new();
        }
        self.message.done = true;
        let item = message_item(&self.response_id, &self.message.text);
        self.completed_items.push((index, item.clone()));
        vec![
            sse_default(
                "response.output_text.done",
                json!({"type":"response.output_text.done",
                    "item_id":self.message.item_id,"output_index":index,"content_index":0,
                    "text":self.message.text}),
            ),
            sse_default(
                "response.content_part.done",
                json!({"type":"response.content_part.done",
                    "item_id":self.message.item_id,"output_index":index,"content_index":0,
                    "part":item["content"][0]}),
            ),
            sse_default(
                "response.output_item.done",
                json!({"type":"response.output_item.done","output_index":index,"item":item}),
            ),
        ]
    }

    pub(super) fn finish_reasoning(&mut self) -> Vec<AdapterEvent> {
        let Some(index) = self.reasoning.output_index else {
            return Vec::new();
        };
        if self.reasoning.done {
            return Vec::new();
        }
        self.reasoning.done = true;
        let item = reasoning_item(&self.response_id, &self.reasoning.text);
        self.completed_items.push((index, item.clone()));
        vec![
            sse_default(
                "response.reasoning_summary_text.done",
                json!({"type":"response.reasoning_summary_text.done",
                    "item_id":self.reasoning.item_id,"output_index":index,
                    "summary_index":0,"text":self.reasoning.text}),
            ),
            sse_default(
                "response.output_item.done",
                json!({"type":"response.output_item.done","output_index":index,"item":item}),
            ),
        ]
    }

    pub(super) fn finish_tools(&mut self) -> Result<Vec<AdapterEvent>, ProtocolError> {
        let mut events = Vec::new();
        for (key, state) in &mut self.tools {
            if state.done {
                continue;
            }
            let index = state
                .output_index
                .ok_or_else(|| invalid("streamed tool call identity is incomplete"))?;
            state.done = true;
            let item = function_call_item(
                &self.response_id,
                *key as usize,
                &state.call_id,
                &state.name,
                &state.arguments,
            );
            self.completed_items.push((index, item.clone()));
            events.push(sse_default(
                "response.function_call_arguments.done",
                json!({"type":"response.function_call_arguments.done","item_id":state.item_id,
                    "output_index":index,"arguments":state.arguments}),
            ));
            events.push(sse_default(
                "response.output_item.done",
                json!({"type":"response.output_item.done","output_index":index,"item":item}),
            ));
        }
        Ok(events)
    }

    pub(super) fn assistant_message(&self) -> Value {
        let mut message = json!({"role":"assistant","content": if self.message.text.is_empty() {
            Value::Null
        } else {
            Value::String(self.message.text.clone())
        }});
        if !self.reasoning.text.is_empty() {
            message["reasoning_content"] = Value::String(self.reasoning.text.clone());
        }
        let calls = self
            .tools
            .values()
            .map(|state| {
                json!({"id":state.call_id,"type":"function",
                    "function":{"name":state.name,"arguments":state.arguments}})
            })
            .collect::<Vec<_>>();
        if !calls.is_empty() {
            message["tool_calls"] = Value::Array(calls);
        }
        message
    }

    fn allocate_output(&mut self) -> usize {
        let index = self.next_output_index;
        self.next_output_index += 1;
        index
    }
}

fn append_fragment(target: &mut String, value: Option<&Value>) {
    if let Some(value) = value.and_then(Value::as_str) {
        target.push_str(value);
    }
}

fn invalid(message: &'static str) -> ProtocolError {
    ProtocolError::InvalidPayload(message.into())
}
