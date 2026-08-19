use std::sync::LazyLock;

use any2api_domain::{ProtocolDialect, ProtocolOperation};
use bytes::Bytes;
use http::HeaderMap;
use serde_json::{Value, json};

use super::prepare;
use crate::api::ProviderRequestContext;

static CLIENT_HEADERS: LazyLock<HeaderMap> = LazyLock::new(HeaderMap::new);

fn context(oauth: bool, operation: ProtocolOperation) -> ProviderRequestContext<'static> {
    ProviderRequestContext {
        ingress_dialect: ProtocolDialect::OpenAiResponses,
        upstream_operation: operation,
        upstream_model: "gpt-5.6-sol",
        client_headers: &CLIENT_HEADERS,
        oauth,
        allow_credential_bound: true,
        allow_session_replay: true,
        allow_turn_state: false,
    }
}

#[test]
fn normalizes_known_codex_oauth_responses_differences() {
    let body = Bytes::from(
        serde_json::to_vec(&json!({
            "model": "gpt-5.6-sol",
            "stream": true,
            "store": true,
            "include": ["file_search_call.results"],
            "parallel_tool_calls": false,
            "max_output_tokens": 64_000,
            "max_completion_tokens": 64_000,
            "temperature": 0.2,
            "top_p": 0.9,
            "service_tier": "standard",
            "truncation": "auto",
            "context_management": {"type": "compaction"},
            "user": "request-owner",
            "unknown": {"nested": [1, 2, 3]},
            "input": [
                {"type": "message", "role": "system", "content": "rules"},
                {"type": "message", "role": "user", "content": "hello"}
            ]
        }))
        .expect("request JSON"),
    );

    let original = body.clone();
    let output = prepare(context(true, ProtocolOperation::Responses), body).expect("normalized");
    let output: Value = serde_json::from_slice(&output).expect("normalized JSON");
    let original: Value = serde_json::from_slice(&original).expect("original JSON");

    assert_eq!(output["store"], false);
    assert_eq!(output["include"], json!(["reasoning.encrypted_content"]));
    assert_eq!(output["parallel_tool_calls"], false);
    assert_eq!(output["input"][0]["role"], "developer");
    assert_eq!(output["input"][1]["role"], "user");
    assert_eq!(output["unknown"], json!({"nested": [1, 2, 3]}));
    assert_eq!(original["store"], true);
    assert_eq!(original["max_output_tokens"], 64_000);
    assert_eq!(original["input"][0]["role"], "system");
    for field in [
        "max_output_tokens",
        "max_completion_tokens",
        "temperature",
        "top_p",
        "service_tier",
        "truncation",
        "context_management",
        "user",
    ] {
        assert!(output.get(field).is_none(), "{field} must be removed");
    }
}

#[test]
fn fills_omitted_defaults_and_wraps_string_input() {
    let body = Bytes::from_static(br#"{"model":"gpt-5.6-sol","input":"hello"}"#);

    let output = prepare(context(true, ProtocolOperation::Responses), body).expect("normalized");
    let output: Value = serde_json::from_slice(&output).expect("normalized JSON");

    assert_eq!(output["store"], false);
    assert_eq!(output["parallel_tool_calls"], true);
    assert_eq!(output["include"], json!(["reasoning.encrypted_content"]));
    assert_eq!(output["input"][0]["type"], "message");
    assert_eq!(output["input"][0]["role"], "user");
    assert_eq!(output["input"][0]["content"][0]["text"], "hello");
}

#[test]
fn replaces_an_invalid_parallel_tool_calls_value() {
    let body = Bytes::from_static(
        br#"{"model":"gpt-5.6-sol","store":false,"parallel_tool_calls":"yes","include":["reasoning.encrypted_content"]}"#,
    );

    let output = prepare(context(true, ProtocolOperation::Responses), body).expect("normalized");
    let output: Value = serde_json::from_slice(&output).expect("normalized JSON");

    assert_eq!(output["parallel_tool_calls"], true);
}

#[test]
fn preserves_priority_and_non_system_content_without_copying() {
    let body = Bytes::from_static(
        br#"{"model":"gpt-5.6-sol","store":false,"parallel_tool_calls":false,"include":["reasoning.encrypted_content"],"service_tier":"priority","input":[{"role":"user","content":"the word system is content"}]}"#,
    );

    let output = prepare(context(true, ProtocolOperation::Responses), body.clone())
        .expect("normalized request");

    assert_eq!(output.as_ptr(), body.as_ptr());
    assert_eq!(output, body);
}

#[test]
fn preserves_the_observed_codex_0_147_responses_shape_without_copying() {
    let body = Bytes::from_static(
        br#"{"model":"gpt-5.6-sol","input":[{"type":"additional_tools","role":"developer","tools":[]},{"type":"message","role":"developer","content":[{"type":"input_text","text":"rules"}]},{"type":"message","role":"user","content":[{"type":"input_text","text":"hello"}]}],"tool_choice":"auto","parallel_tool_calls":false,"reasoning":{"effort":"low","context":"all_turns"},"store":false,"stream":true,"include":["reasoning.encrypted_content"],"prompt_cache_key":"opaque","text":{"verbosity":"low"},"client_metadata":{"session_id":"opaque","x-codex-turn-metadata":"opaque"}}"#,
    );

    let output = prepare(context(true, ProtocolOperation::Responses), body.clone())
        .expect("observed Codex request shape");

    assert_eq!(output.as_ptr(), body.as_ptr());
    assert_eq!(output, body);
}

#[test]
fn rewrites_system_role_when_json_contains_whitespace() {
    let body = Bytes::from_static(
        br#"{"model":"gpt-5.6-sol","store":false,"parallel_tool_calls":false,"include":["reasoning.encrypted_content"],"input":[{"role" : "system","content":"rules"}]}"#,
    );

    let output = prepare(context(true, ProtocolOperation::Responses), body).expect("normalized");
    let output: Value = serde_json::from_slice(&output).expect("normalized JSON");

    assert_eq!(output["input"][0]["role"], "developer");
}

#[test]
fn leaves_api_keys_and_other_operations_byte_identical() {
    let body =
        Bytes::from_static(br#"{"model":"gpt-5.6-sol","store":true,"max_output_tokens":64000}"#);
    for context in [
        context(false, ProtocolOperation::Responses),
        context(true, ProtocolOperation::ResponsesCompact),
        context(true, ProtocolOperation::ChatCompletions),
    ] {
        let output = prepare(context, body.clone()).expect("unchanged request");
        assert_eq!(output.as_ptr(), body.as_ptr());
        assert_eq!(output, body);
    }
}

#[test]
fn normalizes_a_large_body_without_changing_its_payload() {
    let content = "x".repeat(512 * 1024);
    let body = Bytes::from(
        serde_json::to_vec(&json!({
            "model": "gpt-5.6-sol",
            "store": true,
            "input": [{"role": "user", "content": content}]
        }))
        .expect("request JSON"),
    );

    let output = prepare(context(true, ProtocolOperation::Responses), body).expect("normalized");
    assert!(output.len() >= 512 * 1024);
    let output: Value = serde_json::from_slice(&output).expect("normalized JSON");
    assert_eq!(output["input"][0]["content"], content);
    assert_eq!(output["store"], false);
}

#[test]
fn oauth_responses_request_contract_has_an_explicit_wire_golden() {
    let body = Bytes::from_static(
        br#" {"z":{"opaque" : true},"user":"removed","model":"gpt-5.6-sol","input":[{"content":"rules","role" : "system","type":"message"}],"temperature":0.7} "#,
    );

    let output = prepare(context(true, ProtocolOperation::Responses), body)
        .expect("Codex OAuth Responses contract");

    assert_eq!(
        output,
        Bytes::from_static(
            br#"{"input":[{"content":"rules","role":"developer","type":"message"}],"model":"gpt-5.6-sol","z":{"opaque" : true},"store":false,"include":["reasoning.encrypted_content"],"parallel_tool_calls":true}"#
        )
    );
}

fn memory_marked_body(store: bool) -> Bytes {
    Bytes::from(
        serde_json::to_vec(&json!({
            "model": "gpt-5.6-sol",
            "instructions": "memory-instructions",
            "input": [{"type": "message", "role": "user", "content": "rollout"}],
            "reasoning": {"effort": "low"},
            "store": store,
            "prompt_cache_key": "task-session",
            "user": "request-owner",
            "client_metadata": {
                "session_id": "task-session",
                "x-codex-turn-metadata": "{\"request_kind\":\"memory\"}"
            }
        }))
        .expect("request JSON"),
    )
}

#[test]
fn api_key_memory_requests_get_the_derived_cache_key_without_oauth_normalization() {
    let output = prepare(
        context(false, ProtocolOperation::Responses),
        memory_marked_body(true),
    )
    .expect("stabilized request");
    let output: Value = serde_json::from_slice(&output).expect("stabilized JSON");

    assert_eq!(
        output["prompt_cache_key"],
        "860acffa-b5ad-4192-ac67-1708a8316480"
    );
    assert!(output.get("instructions").is_none());
    assert_eq!(output["input"][0]["role"], "developer");
    assert_eq!(output["input"][0]["content"][0]["type"], "input_text");
    assert_eq!(
        output["input"][0]["content"][0]["text"],
        "memory-instructions"
    );
    assert_eq!(
        output["input"][0]["content"][0]["prompt_cache_breakpoint"],
        json!({"mode": "explicit"})
    );
    assert_eq!(output["input"][1]["role"], "user");
    assert_eq!(output["input"][1]["content"], "rollout");
    assert_eq!(output["prompt_cache_options"], json!({"mode": "explicit"}));
    assert_eq!(output["store"], true);
    assert_eq!(output["user"], "request-owner");
    assert!(output.get("include").is_none());
}

#[test]
fn oauth_memory_requests_combine_the_profile_with_the_derived_cache_key() {
    let output = prepare(
        context(true, ProtocolOperation::Responses),
        memory_marked_body(true),
    )
    .expect("normalized request");
    let output: Value = serde_json::from_slice(&output).expect("normalized JSON");

    assert_eq!(
        output["prompt_cache_key"],
        "860acffa-b5ad-4192-ac67-1708a8316480"
    );
    assert!(output.get("instructions").is_none());
    assert_eq!(output["input"][0]["role"], "developer");
    assert_eq!(
        output["input"][0]["content"][0]["text"],
        "memory-instructions"
    );
    assert_eq!(
        output["input"][0]["content"][0]["prompt_cache_breakpoint"],
        json!({"mode": "explicit"})
    );
    assert_eq!(output["input"][1]["role"], "user");
    assert_eq!(output["input"][1]["content"], "rollout");
    assert_eq!(output["prompt_cache_options"], json!({"mode": "explicit"}));
    assert_eq!(output["store"], false);
    assert_eq!(output["include"], json!(["reasoning.encrypted_content"]));
    assert!(output.get("user").is_none());
}

#[test]
fn ordinary_turn_with_memory_shaped_fields_is_left_untouched() {
    let body = Bytes::from(
        serde_json::to_vec(&json!({
            "model": "gpt-5.6-sol",
            "instructions": "turn instructions",
            "input": "turn rollout",
            "prompt_cache_key": "turn-key",
            "prompt_cache_options": {"mode": "implicit"},
            "client_metadata": {
                "x-codex-turn-metadata": "{\"request_kind\":\"turn\"}"
            }
        }))
        .expect("request JSON"),
    );
    let output =
        prepare(context(false, ProtocolOperation::Responses), body.clone()).expect("ordinary turn");
    assert_eq!(output.as_ptr(), body.as_ptr());
    assert_eq!(output, body);
}

#[test]
fn memory_shaped_body_is_untouched_for_non_responses_upstream_operations() {
    let body = memory_marked_body(true);
    for operation in [
        ProtocolOperation::ResponsesCompact,
        ProtocolOperation::ChatCompletions,
    ] {
        let output = prepare(context(false, operation), body.clone()).expect("unchanged request");
        assert_eq!(output.as_ptr(), body.as_ptr());
        assert_eq!(output, body);
    }
}
