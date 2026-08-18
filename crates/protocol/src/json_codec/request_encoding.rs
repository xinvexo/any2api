use any2api_domain::ProtocolOperation;
use any2api_payload_buffer::PayloadBuffer;
use bytes::Bytes;
use serde::{Serialize, Serializer, ser::SerializeMap};
use serde_json::{Map, Value};

use crate::ProtocolError;

pub(super) fn encode(
    operation: ProtocolOperation,
    value: &Value,
    upstream_model: &str,
) -> Result<Bytes, ProtocolError> {
    let object = value.as_object().ok_or_else(|| {
        ProtocolError::InvalidPayload("request body must be a JSON object".into())
    })?;
    let mut body = PayloadBuffer::new(usize::MAX);
    serde_json::to_writer(
        &mut body,
        &BorrowedRequest {
            object,
            upstream_model,
            include_stream: operation.allows_stream(),
        },
    )
    .map_err(|error| {
        if error.is_io() {
            ProtocolError::Internal("protocol payload allocation failed".into())
        } else {
            ProtocolError::InvalidPayload("request JSON could not be encoded".into())
        }
    })?;
    Ok(body.freeze().into_bytes())
}

struct BorrowedRequest<'a> {
    object: &'a Map<String, Value>,
    upstream_model: &'a str,
    include_stream: bool,
}

impl Serialize for BorrowedRequest<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let omitted = usize::from(!self.include_stream && self.object.contains_key("stream"));
        let adds_model = usize::from(!self.object.contains_key("model"));
        let mut map = serializer.serialize_map(Some(self.object.len() - omitted + adds_model))?;
        for (key, value) in self.object {
            if key == "stream" && !self.include_stream {
                continue;
            }
            if key == "model" {
                map.serialize_entry(key, self.upstream_model)?;
            } else {
                map.serialize_entry(key, value)?;
            }
        }
        if adds_model != 0 {
            map.serialize_entry("model", self.upstream_model)?;
        }
        map.end()
    }
}

#[cfg(test)]
mod tests {
    use any2api_domain::ProtocolOperation;
    use serde_json::json;

    use super::encode;

    #[test]
    fn borrowed_encoding_is_repeatable_and_does_not_mutate_the_payload() {
        let payload = json!({
            "model": "public",
            "stream": false,
            "extra": {"nested": [1, 2, 3]}
        });
        let original = payload.clone();

        let first = encode(ProtocolOperation::MessagesCountTokens, &payload, "upstream")
            .expect("first encoding");
        let second = encode(ProtocolOperation::MessagesCountTokens, &payload, "upstream")
            .expect("retry encoding");

        assert_eq!(payload, original);
        assert_eq!(first, second);
        assert_eq!(
            first.as_ref(),
            br#"{"extra":{"nested":[1,2,3]},"model":"upstream"}"#
        );
    }

    #[test]
    fn missing_model_is_added_without_mutating_a_large_payload() {
        let content = "x".repeat(300 * 1024);
        let payload = json!({"input": content, "stream": true});

        let encoded = encode(ProtocolOperation::MessagesCountTokens, &payload, "upstream")
            .expect("large encoding");
        let decoded: serde_json::Value = serde_json::from_slice(&encoded).expect("encoded JSON");

        assert_eq!(decoded["model"], "upstream");
        assert!(decoded.get("stream").is_none());
        assert_eq!(decoded["input"], payload["input"]);
        assert!(payload.get("model").is_none());
    }
}
