use any2api_domain::ProtocolOperation;
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
    let body = if object.contains_key("model") {
        serde_json::to_vec(&BorrowedRequest {
            object,
            upstream_model,
            include_stream: operation.allows_stream(),
        })
    } else {
        encode_missing_model_fallback(operation, value.clone(), upstream_model)
    };
    body.map(Bytes::from)
        .map_err(|_| ProtocolError::InvalidPayload("request JSON could not be encoded".into()))
}

fn encode_missing_model_fallback(
    operation: ProtocolOperation,
    mut value: Value,
    upstream_model: &str,
) -> serde_json::Result<Vec<u8>> {
    let object = value
        .as_object_mut()
        .expect("request object was checked before fallback");
    object.insert("model".into(), Value::String(upstream_model.to_owned()));
    if !operation.allows_stream() {
        object.remove("stream");
    }
    serde_json::to_vec(&value)
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
        let mut map = serializer.serialize_map(Some(self.object.len() - omitted))?;
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
        map.end()
    }
}

#[cfg(test)]
mod tests {
    use std::{hint::black_box, time::Instant};

    use any2api_domain::ProtocolOperation;
    use serde_json::{Map, Value, json};

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
    #[ignore = "manual peak-RSS benchmark; run in an isolated test process"]
    fn large_request_encoding_benchmark() {
        let mode = std::env::var("ANY2API_REQUEST_ENCODING_BENCH_MODE")
            .unwrap_or_else(|_| "shared".into());
        let items = (0..200_000)
            .map(|index| {
                Value::String(format!(
                    "item-{index:08}-abcdefghijklmnopqrstuvwxyz-ABCDEFGHIJKLMNOPQRSTUVWXYZ"
                ))
            })
            .collect();
        let mut object = Map::new();
        object.insert("input".into(), Value::Array(items));
        object.insert("model".into(), Value::String("public".into()));
        object.insert("stream".into(), Value::Bool(false));
        let payload = Value::Object(object);
        let started = Instant::now();
        let encoded = match mode.as_str() {
            "shared" => encode(ProtocolOperation::Responses, &payload, "upstream"),
            "deep-clone" => {
                let cloned: Value = payload.clone();
                encode(ProtocolOperation::Responses, &cloned, "upstream")
            }
            other => panic!("unknown benchmark mode {other}"),
        }
        .expect("large request encoding");
        eprintln!(
            "mode={mode} items={} output_bytes={} elapsed_ms={}",
            payload["input"].as_array().expect("input array").len(),
            encoded.len(),
            started.elapsed().as_millis(),
        );
        black_box(encoded);
    }
}
