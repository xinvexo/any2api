use serde_json::Value;

use crate::api::AdapterPayload;

pub(super) fn normalize(payload: &mut AdapterPayload) {
    let AdapterPayload::Json(value) = payload else {
        return;
    };
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
    use serde_json::json;

    use super::normalize;
    use crate::api::AdapterPayload;

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
        normalize(&mut payload);
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

        normalize(&mut payload);

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
        normalize(&mut payload);
        let AdapterPayload::Json(payload) = payload else {
            panic!("JSON payload");
        };
        assert_eq!(payload, json!({"model": "gpt", "input": "hello"}));
    }
}
