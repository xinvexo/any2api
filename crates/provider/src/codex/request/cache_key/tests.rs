use std::collections::BTreeSet;

use bytes::Bytes;
use serde_json::{Value, json};

use super::stabilize_memory_prompt_cache;

const LUNA_LOW_MEMORY_KEY: &str = "2df97426-013e-4b29-bcbe-ab566447b80a";
const LUNA_ABSENT_EFFORT_MEMORY_KEY: &str = "5c467b5c-433c-469e-997a-2a46ceedf614";

#[test]
fn memory_requests_share_one_instruction_scoped_key_across_sessions() {
    let first = stabilize(memory_body("session-a", "rollout-a"));
    let second = stabilize(memory_body("session-b", "rollout-b"));

    assert_eq!(first["prompt_cache_key"], LUNA_LOW_MEMORY_KEY);
    assert_eq!(second["prompt_cache_key"], LUNA_LOW_MEMORY_KEY);
    assert!(first.get("instructions").is_none());
    assert_eq!(first["input"][0], second["input"][0]);
    assert_eq!(
        first["input"][0]["content"][0]["prompt_cache_breakpoint"],
        json!({"mode": "explicit"})
    );
    assert_eq!(first["prompt_cache_options"], json!({"mode": "explicit"}));
    assert_eq!(first["input"][1]["content"][0]["text"], "rollout-a");
    assert_eq!(second["input"][1]["content"][0]["text"], "rollout-b");
    assert!(
        first["input"][1]["content"][0]
            .get("prompt_cache_breakpoint")
            .is_none()
    );
    assert_ne!(first["input"][1], second["input"][1]);
    assert_eq!(first["client_metadata"]["session_id"], "session-a");
    assert_eq!(first["store"], false);
    assert_eq!(first["stream"], true);
}

#[test]
fn derived_keys_scope_by_model_effort_and_instructions() {
    let base = stabilized_key(memory_body("session", "rollout"));
    let other_model =
        stabilize_memory_prompt_cache("gpt-5.6-sol", memory_body("session", "rollout"))
            .expect("other model");
    let other_model: Value = serde_json::from_slice(&other_model).expect("other model JSON");
    let other_model = other_model["prompt_cache_key"]
        .as_str()
        .expect("other model key")
        .to_owned();
    let other_effort = stabilized_key(custom_memory_body(|value| {
        value["reasoning"]["effort"] = json!("medium");
    }));
    let absent_effort = stabilized_key(custom_memory_body(|value| {
        value.as_object_mut().expect("object").remove("reasoning");
    }));
    let other_instructions = stabilized_key(custom_memory_body(|value| {
        value["instructions"] = json!("consolidation-instructions");
    }));

    assert_eq!(base, LUNA_LOW_MEMORY_KEY);
    assert_eq!(absent_effort, LUNA_ABSENT_EFFORT_MEMORY_KEY);
    let distinct = BTreeSet::from([
        base,
        other_model,
        other_effort,
        absent_effort,
        other_instructions,
    ]);
    assert_eq!(distinct.len(), 5);
}

#[test]
fn ambiguous_or_non_memory_requests_pass_through_byte_identical() {
    let cases = [
        custom_memory_body(|value| {
            value["client_metadata"]["x-codex-turn-metadata"] = json!(turn_metadata("turn"));
        }),
        custom_memory_body(|value| {
            value["client_metadata"]["x-codex-turn-metadata"] = json!(turn_metadata("prewarm"));
        }),
        custom_memory_body(|value| {
            value["client_metadata"]["x-codex-turn-metadata"] = json!(turn_metadata("compact"));
        }),
        custom_memory_body(|value| {
            value["client_metadata"]["x-codex-turn-metadata"] = json!(turn_metadata("review"));
        }),
        custom_memory_body(|value| {
            value
                .as_object_mut()
                .expect("object")
                .remove("client_metadata");
        }),
        custom_memory_body(|value| {
            value["client_metadata"] = json!("x-codex-turn-metadata");
        }),
        custom_memory_body(|value| {
            value["client_metadata"] = json!({"session_id": "x-codex-turn-metadata"});
        }),
        custom_memory_body(|value| {
            value["client_metadata"]["x-codex-turn-metadata"] = json!({"request_kind": "memory"});
        }),
        custom_memory_body(|value| {
            value["client_metadata"]["x-codex-turn-metadata"] = json!("not structured metadata");
        }),
        custom_memory_body(|value| {
            value["client_metadata"]["x-codex-turn-metadata"] =
                json!(serde_json::to_string(&json!({"request_kind": 42})).expect("metadata"));
        }),
        custom_memory_body(|value| {
            value
                .as_object_mut()
                .expect("object")
                .remove("instructions");
        }),
        custom_memory_body(|value| {
            value["instructions"] = json!("");
        }),
        custom_memory_body(|value| {
            value["instructions"] = json!(["memory-instructions"]);
        }),
        custom_memory_body(|value| {
            value["input"] = json!({"type": "message", "role": "user"});
        }),
        Bytes::from_static(br#"{"client_metadata":{"x-codex-turn-metadata":"#),
    ];

    for (index, body) in cases.into_iter().enumerate() {
        let output =
            stabilize_memory_prompt_cache("gpt-5.6-luna", body.clone()).expect("passthrough");
        assert_eq!(output.as_ptr(), body.as_ptr(), "case {index} must not copy");
        assert_eq!(output, body, "case {index} must pass through");
    }
}

#[test]
fn missing_prompt_cache_key_is_added_for_memory_requests() {
    let output = stabilize(custom_memory_body(|value| {
        value
            .as_object_mut()
            .expect("object")
            .remove("prompt_cache_key");
    }));

    assert_eq!(output["prompt_cache_key"], LUNA_LOW_MEMORY_KEY);
}

#[test]
fn memory_consolidation_requests_are_stabilized_too() {
    let output = stabilize(custom_memory_body(|value| {
        value["client_metadata"]["x-codex-turn-metadata"] =
            json!(turn_metadata("memory_consolidation"));
    }));

    assert_eq!(output["prompt_cache_key"], LUNA_LOW_MEMORY_KEY);
}

#[test]
fn existing_prompt_cache_fields_are_rewritten_deterministically() {
    let output = stabilize(custom_memory_body(|value| {
        value["prompt_cache_key"] = json!("session-specific-key");
        value["prompt_cache_options"] = json!({"mode": "implicit", "ttl": "30m"});
    }));

    assert_eq!(output["prompt_cache_key"], LUNA_LOW_MEMORY_KEY);
    assert_eq!(
        output["prompt_cache_options"],
        json!({"mode": "explicit", "ttl": "30m"})
    );
}

#[test]
fn unsupported_gpt_models_pass_through_without_memory_rewrite() {
    for model in ["gpt-5.5", "gpt-5.60", "custom-gpt-5.6-luna"] {
        let body = custom_memory_body(|value| value["model"] = json!(model));
        let output = stabilize_memory_prompt_cache(model, body.clone()).expect("passthrough");
        assert_eq!(output, body, "model {model} must pass through");
    }
}

#[test]
fn malformed_existing_prompt_cache_options_fail_closed() {
    let body = custom_memory_body(|value| value["prompt_cache_options"] = json!("implicit"));
    let output = stabilize_memory_prompt_cache("gpt-5.6-luna", body.clone()).expect("passthrough");
    assert_eq!(output.as_ptr(), body.as_ptr());
    assert_eq!(output, body);
}

#[test]
fn string_input_is_wrapped_after_the_breakpoint() {
    let output = stabilize(custom_memory_body(|value| {
        value["input"] = json!("rollout")
    }));
    assert_eq!(output["input"].as_array().expect("input array").len(), 2);
    assert_eq!(output["input"][0]["role"], "developer");
    assert_eq!(output["input"][1]["role"], "user");
    assert_eq!(output["input"][1]["content"][0]["type"], "input_text");
    assert_eq!(output["input"][1]["content"][0]["text"], "rollout");
}

fn stabilize(body: Bytes) -> Value {
    let output = stabilize_memory_prompt_cache("gpt-5.6-luna", body).expect("stabilized");
    serde_json::from_slice(&output).expect("stabilized JSON")
}

fn stabilized_key(body: Bytes) -> String {
    stabilize(body)["prompt_cache_key"]
        .as_str()
        .expect("derived key")
        .to_owned()
}

fn memory_body(session: &str, rollout: &str) -> Bytes {
    custom_memory_body(|value| {
        value["prompt_cache_key"] = json!(session);
        value["client_metadata"]["session_id"] = json!(session);
        value["input"] = json!(rollout);
    })
}

fn custom_memory_body(mutate: impl FnOnce(&mut Value)) -> Bytes {
    let mut value = json!({
        "model": "gpt-5.6-luna",
        "instructions": "memory-instructions",
        "input": "rollout",
        "reasoning": {"effort": "low", "context": "all_turns"},
        "store": false,
        "stream": true,
        "prompt_cache_key": "session-key",
        "client_metadata": {
            "session_id": "session-key",
            "x-codex-turn-metadata": turn_metadata("memory"),
        }
    });
    mutate(&mut value);
    Bytes::from(serde_json::to_vec(&value).expect("request body"))
}

fn turn_metadata(request_kind: &str) -> String {
    serde_json::to_string(&json!({
        "request_kind": request_kind,
        "session_id": "session-key",
        "phase": "implementation",
        "turn_started_at_unix_ms": 1_755_200_000_000_i64,
    }))
    .expect("turn metadata")
}
