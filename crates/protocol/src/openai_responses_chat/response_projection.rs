use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value, json};

#[derive(Clone)]
pub(super) struct ResponseProjection {
    response_id: String,
    created_at: u64,
    upstream_model: String,
    request_fields: Map<String, Value>,
}

impl ResponseProjection {
    pub(super) fn new(response_id: String, upstream_model: String, request: &Value) -> Self {
        const ECHO_FIELDS: &[&str] = &[
            "instructions",
            "max_output_tokens",
            "parallel_tool_calls",
            "previous_response_id",
            "reasoning",
            "text",
            "tool_choice",
            "tools",
            "temperature",
            "top_p",
            "service_tier",
            "store",
            "metadata",
        ];
        let mut request_fields = Map::new();
        if let Some(source) = request.as_object() {
            for field in ECHO_FIELDS {
                if let Some(value) = source.get(*field) {
                    request_fields.insert((*field).to_owned(), value.clone());
                }
            }
        }
        request_fields
            .entry("parallel_tool_calls")
            .or_insert(Value::Bool(true));
        request_fields
            .entry("tools")
            .or_insert_with(|| Value::Array(Vec::new()));

        Self {
            response_id,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_secs()),
            upstream_model,
            request_fields,
        }
    }

    pub(super) fn response_id(&self) -> &str {
        &self.response_id
    }

    pub(super) const fn created_at(&self) -> u64 {
        self.created_at
    }

    pub(super) fn upstream_model(&self) -> &str {
        &self.upstream_model
    }

    pub(super) fn base_response(
        &self,
        status: &str,
        output: Vec<Value>,
        created_at: u64,
        model: &str,
        usage: Value,
    ) -> Value {
        let mut response = json!({
            "id":self.response_id,
            "object":"response",
            "created_at":created_at,
            "status":status,
            "model":model,
            "output":output,
            "error":Value::Null,
            "incomplete_details":Value::Null,
            "usage":usage
        });
        let object = response
            .as_object_mut()
            .expect("base Responses projection is an object");
        object.extend(self.request_fields.clone());
        response
    }
}
