use std::fmt::Display;

use serde_json::{Value, json};

use crate::openai_responses_chat::tool_projection::RestoredToolCall;

pub(in crate::openai_responses_chat) fn message_item(response_id: &str, text: &str) -> Value {
    json!({
        "id":message_item_id(response_id),"type":"message","status":"completed",
        "role":"assistant","content":[{"type":"output_text","text":text,"annotations":[]}]
    })
}

pub(in crate::openai_responses_chat) fn reasoning_item(response_id: &str, text: &str) -> Value {
    json!({
        "id":reasoning_item_id(response_id),"type":"reasoning","status":"completed",
        "summary":[{"type":"summary_text","text":text}]
    })
}

pub(in crate::openai_responses_chat) fn restored_call_item(
    response_id: &str,
    index: impl Display,
    call_id: &str,
    call: &RestoredToolCall,
    status: &str,
) -> Value {
    match call {
        RestoredToolCall::Function {
            name,
            namespace,
            arguments,
        } => {
            let mut item = json!({
                "id":function_call_item_id(response_id,index),"type":"function_call",
                "status":status,"call_id":call_id,"name":name,"arguments":arguments
            });
            if let Some(namespace) = namespace {
                item["namespace"] = Value::String(namespace.clone());
            }
            item
        }
        RestoredToolCall::Custom {
            name,
            namespace,
            input,
        } => {
            let mut item = json!({
                "id":custom_call_item_id(response_id,index),"type":"custom_tool_call",
                "status":status,"call_id":call_id,"name":name,"input":input
            });
            if let Some(namespace) = namespace {
                item["namespace"] = Value::String(namespace.clone());
            }
            item
        }
        RestoredToolCall::ToolSearch { arguments } => json!({
            "id":tool_search_call_item_id(response_id,index),"type":"tool_search_call",
            "status":status,"call_id":call_id,"execution":"client","arguments":arguments
        }),
    }
}

pub(in crate::openai_responses_chat) fn message_item_id(response_id: &str) -> String {
    format!("msg_{response_id}")
}

pub(in crate::openai_responses_chat) fn reasoning_item_id(response_id: &str) -> String {
    format!("rs_{response_id}")
}

pub(in crate::openai_responses_chat) fn function_call_item_id(
    response_id: &str,
    index: impl Display,
) -> String {
    format!("fc_{response_id}_{index}")
}

pub(in crate::openai_responses_chat) fn custom_call_item_id(
    response_id: &str,
    index: impl Display,
) -> String {
    format!("ctc_{response_id}_{index}")
}

pub(in crate::openai_responses_chat) fn tool_search_call_item_id(
    response_id: &str,
    index: impl Display,
) -> String {
    format!("tsc_{response_id}_{index}")
}
