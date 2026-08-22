use serde_json::json;

use super::super::wire::{SynthesizedEvent, content_telemetry, event, event_default};
use super::ChatToResponsesStream;
use crate::openai_responses_chat::response::{
    message_item, message_item_id, reasoning_item, reasoning_item_id,
};

#[derive(Default)]
pub(super) struct TextState {
    pub(super) output_index: Option<usize>,
    pub(super) item_id: String,
    pub(super) text: String,
    pub(super) done: bool,
}

impl ChatToResponsesStream {
    pub(super) fn push_text(&mut self, delta: &str) -> Vec<SynthesizedEvent> {
        if delta.is_empty() {
            return Vec::new();
        }
        let mut events = Vec::new();
        if self.message.output_index.is_none() {
            let index = self.allocate_output();
            self.message.output_index = Some(index);
            self.message.item_id = message_item_id(self.response_id());
            events.push(event_default(
                "response.output_item.added",
                json!({"type":"response.output_item.added","output_index":index,"item":{
                    "id":self.message.item_id,"type":"message","status":"in_progress",
                    "role":"assistant","content":[]
                }}),
            ));
            events.push(event_default(
                "response.content_part.added",
                json!({"type":"response.content_part.added","item_id":self.message.item_id,
                    "output_index":index,"content_index":0,
                    "part":{"type":"output_text","text":"","annotations":[]}}),
            ));
        }
        self.message.text.push_str(delta);
        events.push(event(
            "response.output_text.delta",
            json!({"type":"response.output_text.delta","item_id":self.message.item_id,
                "output_index":self.message.output_index.unwrap_or(0),"content_index":0,
                "delta":delta}),
            content_telemetry(),
        ));
        events
    }

    pub(super) fn push_reasoning(&mut self, delta: &str) -> Vec<SynthesizedEvent> {
        if delta.is_empty() {
            return Vec::new();
        }
        let mut events = Vec::new();
        if self.reasoning.output_index.is_none() {
            let index = self.allocate_output();
            self.reasoning.output_index = Some(index);
            self.reasoning.item_id = reasoning_item_id(self.response_id());
            events.push(event_default(
                "response.output_item.added",
                json!({"type":"response.output_item.added","output_index":index,"item":{
                    "id":self.reasoning.item_id,"type":"reasoning","status":"in_progress",
                    "summary":[]
                }}),
            ));
            events.push(event_default(
                "response.reasoning_summary_part.added",
                json!({"type":"response.reasoning_summary_part.added",
                    "item_id":self.reasoning.item_id,"output_index":index,"summary_index":0,
                    "part":{"type":"summary_text","text":""}}),
            ));
        }
        self.reasoning.text.push_str(delta);
        events.push(event(
            "response.reasoning_summary_text.delta",
            json!({"type":"response.reasoning_summary_text.delta",
                "item_id":self.reasoning.item_id,
                "output_index":self.reasoning.output_index.unwrap_or(0),
                "summary_index":0,"delta":delta}),
            content_telemetry(),
        ));
        events
    }

    pub(super) fn finish_message(&mut self) -> Vec<SynthesizedEvent> {
        let Some(index) = self.message.output_index else {
            return Vec::new();
        };
        if self.message.done {
            return Vec::new();
        }
        self.message.done = true;
        let item = message_item(self.response_id(), &self.message.text);
        self.completed_items.push((index, item.clone()));
        vec![
            event_default(
                "response.output_text.done",
                json!({"type":"response.output_text.done",
                    "item_id":self.message.item_id,"output_index":index,"content_index":0,
                    "text":self.message.text}),
            ),
            event_default(
                "response.content_part.done",
                json!({"type":"response.content_part.done",
                    "item_id":self.message.item_id,"output_index":index,"content_index":0,
                    "part":item["content"][0]}),
            ),
            event_default(
                "response.output_item.done",
                json!({"type":"response.output_item.done","output_index":index,"item":item}),
            ),
        ]
    }

    pub(super) fn finish_reasoning(&mut self) -> Vec<SynthesizedEvent> {
        let Some(index) = self.reasoning.output_index else {
            return Vec::new();
        };
        if self.reasoning.done {
            return Vec::new();
        }
        self.reasoning.done = true;
        let item = reasoning_item(self.response_id(), &self.reasoning.text);
        self.completed_items.push((index, item.clone()));
        vec![
            event_default(
                "response.reasoning_summary_text.done",
                json!({"type":"response.reasoning_summary_text.done",
                    "item_id":self.reasoning.item_id,"output_index":index,
                    "summary_index":0,"text":self.reasoning.text}),
            ),
            event_default(
                "response.reasoning_summary_part.done",
                json!({"type":"response.reasoning_summary_part.done",
                    "item_id":self.reasoning.item_id,"output_index":index,
                    "summary_index":0,"part":item["summary"][0]}),
            ),
            event_default(
                "response.output_item.done",
                json!({"type":"response.output_item.done","output_index":index,"item":item}),
            ),
        ]
    }

    pub(super) fn allocate_output(&mut self) -> usize {
        let index = self.next_output_index;
        self.next_output_index += 1;
        index
    }
}
