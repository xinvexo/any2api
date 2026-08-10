use std::borrow::Cow;

use serde::Deserialize;
use serde_json::{Value, value::RawValue};

use crate::{
    ProtocolError,
    api::{AdapterPayload, RawJsonPayload},
    raw_json::{borrowed_object, raw_array},
};

pub(super) fn normalize(payload: &mut AdapterPayload) -> Result<(), ProtocolError> {
    match payload {
        AdapterPayload::Json(value) => normalize_value(value),
        AdapterPayload::RawJson(value) => normalize_raw(value)?,
        AdapterPayload::Multipart(_) => {}
    }
    Ok(())
}

fn normalize_value(value: &mut Value) {
    let Some(input) = value
        .as_object_mut()
        .and_then(|request| request.get_mut("input"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };

    for item in input {
        normalize_item(item);
    }
}

fn normalize_raw(payload: &mut RawJsonPayload) -> Result<(), ProtocolError> {
    let Some(input) = payload.field("input") else {
        return Ok(());
    };
    let Some(items) = raw_array(input) else {
        return Ok(());
    };
    if !items.iter().copied().any(raw_item_has_invalid_id) {
        return Ok(());
    }

    let mut normalized = Vec::with_capacity(input.len());
    normalized.push(b'[');
    for (index, item) in items.iter().enumerate() {
        if index != 0 {
            normalized.push(b',');
        }
        if raw_item_has_invalid_id(item) {
            write_item_without_id(&mut normalized, item)?;
        } else {
            normalized.extend_from_slice(item.get().as_bytes());
        }
    }
    normalized.push(b']');
    payload.replace_raw_field("input", &normalized)
}

#[derive(Deserialize)]
struct RawItemIdentity<'a> {
    #[serde(rename = "type", borrow)]
    item_type: Option<Cow<'a, str>>,
    #[serde(borrow)]
    id: Option<&'a RawValue>,
}

fn raw_item_has_invalid_id(item: &RawValue) -> bool {
    let Ok(identity) = serde_json::from_str::<RawItemIdentity<'_>>(item.get()) else {
        return false;
    };
    let Some(prefixes) = identity.item_type.as_deref().and_then(allowed_id_prefixes) else {
        return false;
    };
    identity
        .id
        .and_then(|id| serde_json::from_str::<Cow<'_, str>>(id.get()).ok())
        .is_some_and(|id| !has_allowed_id_prefix(&id, prefixes))
}

fn write_item_without_id(encoded: &mut Vec<u8>, item: &RawValue) -> Result<(), ProtocolError> {
    let fields = borrowed_object(item.get().as_bytes()).map_err(|_| {
        ProtocolError::InvalidPayload("Responses input item must be valid JSON".into())
    })?;
    encoded.push(b'{');
    let mut first = true;
    for (name, value) in fields {
        if name == "id" {
            continue;
        }
        if !first {
            encoded.push(b',');
        }
        first = false;
        serde_json::to_writer(&mut *encoded, &name).map_err(|_| {
            ProtocolError::InvalidPayload("Responses input item could not be encoded".into())
        })?;
        encoded.push(b':');
        encoded.extend_from_slice(value.get().as_bytes());
    }
    encoded.push(b'}');
    Ok(())
}

fn normalize_item(item: &mut Value) {
    let Some(item) = item.as_object_mut() else {
        return;
    };
    let Some(prefixes) = item
        .get("type")
        .and_then(Value::as_str)
        .and_then(allowed_id_prefixes)
    else {
        return;
    };
    let has_invalid_id = item
        .get("id")
        .and_then(Value::as_str)
        .is_some_and(|id| !has_allowed_id_prefix(id, prefixes));
    if has_invalid_id {
        item.remove("id");
    }
}

fn has_allowed_id_prefix(id: &str, allowed: &[&str]) -> bool {
    id.split_once('_')
        .is_some_and(|(prefix, suffix)| !suffix.is_empty() && allowed.contains(&prefix))
}

fn allowed_id_prefixes(item_type: &str) -> Option<&'static [&'static str]> {
    match item_type {
        "additional_tools" => Some(&["at"]),
        "message" => Some(&["msg"]),
        "agent_message" => Some(&["amsg"]),
        "reasoning" => Some(&["rs"]),
        "local_shell_call" => Some(&["lsh"]),
        "function_call" => Some(&["fc"]),
        "tool_search_call" => Some(&["tsc"]),
        "function_call_output" => Some(&["fco", "fc"]),
        "custom_tool_call" => Some(&["ctc"]),
        "custom_tool_call_output" => Some(&["ctco"]),
        "tool_search_output" => Some(&["tso"]),
        "web_search_call" => Some(&["ws"]),
        "image_generation_call" => Some(&["ig"]),
        "compaction" | "compaction_summary" | "context_compaction" => Some(&["cmp"]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use any2api_domain::ProtocolOperation;
    use bytes::Bytes;
    use serde_json::json;

    use super::normalize;
    use crate::api::{AdapterPayload, RawJsonPayload};

    #[test]
    fn normalizes_every_known_item_type_and_keeps_allowed_prefixes() {
        let cases = [
            ("additional_tools", "at"),
            ("message", "msg"),
            ("agent_message", "amsg"),
            ("reasoning", "rs"),
            ("local_shell_call", "lsh"),
            ("function_call", "fc"),
            ("tool_search_call", "tsc"),
            ("function_call_output", "fco"),
            ("custom_tool_call", "ctc"),
            ("custom_tool_call_output", "ctco"),
            ("tool_search_output", "tso"),
            ("web_search_call", "ws"),
            ("image_generation_call", "ig"),
            ("compaction", "cmp"),
            ("compaction_summary", "cmp"),
            ("context_compaction", "cmp"),
        ];
        let mut input = Vec::new();
        for (item_type, prefix) in cases {
            input.push(json!({"type": item_type, "id": "item_wrong"}));
            input.push(json!({"type": item_type, "id": format!("{prefix}_valid")}));
        }
        input.push(json!({"type": "function_call_output", "id": "fc_valid"}));

        let mut payload = AdapterPayload::Json(json!({"model": "gpt", "input": input}));
        normalize(&mut payload).expect("normalize");
        let AdapterPayload::Json(payload) = payload else {
            panic!("JSON payload");
        };
        let normalized = payload["input"].as_array().expect("input array");
        for pair in normalized[..normalized.len() - 1].chunks_exact(2) {
            assert!(pair[0].get("id").is_none());
            assert!(
                pair[1]["id"]
                    .as_str()
                    .is_some_and(|id| id.ends_with("_valid"))
            );
        }
        assert_eq!(normalized.last().expect("fc output")["id"], "fc_valid");
    }

    #[test]
    fn preserves_state_references_opaque_fields_and_non_string_ids() {
        let original = json!({
            "model": "gpt",
            "previous_response_id": "resp_server_state",
            "input": [
                {
                    "type": "reasoning",
                    "id": "item_wrong",
                    "encrypted_content": "opaque",
                    "summary": [{"type": "summary_text", "text": "summary"}],
                    "metadata": {"id": "item_nested"}
                },
                {"type": "item_reference", "id": "item_server_state"},
                {"type": "future_item", "id": "item_future"},
                {"type": "function_call", "id": 7, "call_id": "call_semantic"},
                {"type": "message", "id": "msg_", "role": "assistant", "content": []}
            ]
        });
        let mut payload = AdapterPayload::Json(original);

        normalize(&mut payload).expect("normalize");

        let AdapterPayload::Json(payload) = payload else {
            panic!("JSON payload");
        };
        assert_eq!(payload["previous_response_id"], "resp_server_state");
        assert!(payload["input"][0].get("id").is_none());
        assert_eq!(payload["input"][0]["encrypted_content"], "opaque");
        assert_eq!(payload["input"][0]["metadata"]["id"], "item_nested");
        assert_eq!(payload["input"][1]["id"], "item_server_state");
        assert_eq!(payload["input"][2]["id"], "item_future");
        assert_eq!(payload["input"][3]["id"], 7);
        assert_eq!(payload["input"][3]["call_id"], "call_semantic");
        assert!(payload["input"][4].get("id").is_none());
    }

    #[test]
    fn leaves_non_array_input_unchanged() {
        let mut payload = AdapterPayload::Json(json!({"model": "gpt", "input": "hello"}));
        normalize(&mut payload).expect("normalize");
        let AdapterPayload::Json(payload) = payload else {
            panic!("JSON payload");
        };
        assert_eq!(payload, json!({"model": "gpt", "input": "hello"}));
    }

    #[test]
    fn raw_payload_normalization_preserves_opaque_fields_without_materializing_the_request() {
        let body = Bytes::from_static(
            br#"{
                "model":"gpt",
                "input":[
                    {"type":"message","id":"item_wrong","content":[],"opaque":{"x":1}},
                    {"type":"reasoning","id":"rs_valid","summary":[]},
                    {"type":"future_item","id":"item_future"}
                ],
                "future_top_level":{"preserve":true}
            }"#,
        );
        let mut payload =
            AdapterPayload::RawJson(RawJsonPayload::parse(body).expect("raw Responses request"));

        normalize(&mut payload).expect("normalize raw request");

        let AdapterPayload::RawJson(payload) = payload else {
            panic!("raw JSON payload");
        };
        let encoded = payload
            .encode(ProtocolOperation::Responses, "gpt")
            .expect("encode normalized request");
        let value: serde_json::Value = serde_json::from_slice(&encoded).expect("request JSON");
        assert!(value["input"][0].get("id").is_none());
        assert_eq!(value["input"][0]["opaque"]["x"], 1);
        assert_eq!(value["input"][1]["id"], "rs_valid");
        assert_eq!(value["input"][2]["id"], "item_future");
        assert_eq!(value["future_top_level"]["preserve"], true);
    }

    #[test]
    fn replay_identity_materialization_has_an_explicit_wire_golden() {
        let body = Bytes::from_static(
            br#"{
                "z":{"keep" : true},
                "model":"gpt",
                "input":[
                    {"z":1,"type":"message","id":"item_wrong","content":[],"a":2},
                    {"type" : "reasoning", "id":"rs_valid", "summary":[]}
                ],
                "a":0
            }"#,
        );
        let mut payload =
            AdapterPayload::RawJson(RawJsonPayload::parse(body).expect("raw Responses request"));

        normalize(&mut payload).expect("normalize replay identity");

        let AdapterPayload::RawJson(payload) = payload else {
            panic!("raw JSON payload");
        };
        assert_eq!(
            payload
                .encode(ProtocolOperation::Responses, "gpt")
                .expect("encode normalized request"),
            Bytes::from_static(
                br#"{"a":0,"input":[{"a":2,"content":[],"type":"message","z":1},{"type" : "reasoning", "id":"rs_valid", "summary":[]}],"model":"gpt","z":{"keep" : true}}"#
            )
        );
    }

    #[test]
    fn valid_replay_identity_preserves_the_original_wire_allocation() {
        let body = Bytes::from_static(
            br#" { "model" : "gpt", "input" : [{"type":"message","id":"msg_valid","content":[]}], "future": true } "#,
        );
        let mut payload = AdapterPayload::RawJson(
            RawJsonPayload::parse(body.clone()).expect("raw Responses request"),
        );

        normalize(&mut payload).expect("valid identity needs no rewrite");

        let AdapterPayload::RawJson(payload) = payload else {
            panic!("raw JSON payload");
        };
        let encoded = payload
            .encode(ProtocolOperation::Responses, "gpt")
            .expect("direct request");
        assert_eq!(encoded, body);
        assert_eq!(encoded.as_ptr(), body.as_ptr());
    }
}
