use any2api_domain::ProtocolOperation;
use serde_json::{Value, json};

use crate::api::BridgeContinuationState;

use super::{bridged_exchange, decoded, registry, upstream_response};

#[tokio::test]
async fn json_bridge_converts_tools_usage_and_previous_response_history() {
    let registry = registry();
    let first = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({
            "model":"public-model",
            "instructions":"Be concise",
            "input":"What is the weather?",
            "reasoning":{"effort":"medium","summary":"concise"},
            "include":["reasoning.encrypted_content"],
            "text":{"format":{
                "type":"json_schema",
                "name":"weather_result",
                "schema":{"type":"object","properties":{"city":{"type":"string"}}},
                "strict":true
            }},
            "tools":[{
                "type":"function",
                "name":"weather",
                "description":"Read weather",
                "parameters":{"type":"object","properties":{"city":{"type":"string"}}},
                "strict":true
            }],
            "tool_choice":{"type":"function","name":"weather"}
        }),
    )
    .await;
    let mut exchange = bridged_exchange(&registry, ProtocolOperation::Responses);
    let prepared = exchange
        .prepare_request(first, "upstream-model", None)
        .expect("bridged request");
    assert_eq!(
        prepared.upstream_operation,
        ProtocolOperation::ChatCompletions
    );
    let upstream_request: Value =
        serde_json::from_slice(&prepared.request.body).expect("upstream request JSON");
    assert_eq!(upstream_request["model"], "upstream-model");
    assert_eq!(upstream_request["messages"][0]["role"], "system");
    assert_eq!(
        upstream_request["messages"][1]["content"],
        "What is the weather?"
    );
    assert_eq!(upstream_request["reasoning_effort"], "medium");
    assert!(upstream_request.get("reasoning").is_none());
    assert!(upstream_request.get("include").is_none());
    assert_eq!(upstream_request["response_format"]["type"], "json_schema");
    assert_eq!(upstream_request["tools"][0]["function"]["name"], "weather");
    assert_eq!(
        upstream_request["tool_choice"]["function"]["name"],
        "weather"
    );

    let decoded_response = exchange
        .decode_upstream_response(upstream_response(json!({
            "id":"chatcmpl_1",
            "created":123,
            "model":"upstream-model",
            "choices":[{
                "index":0,
                "message":{
                    "role":"assistant",
                    "content":"I will check.",
                    "reasoning_content":"Need the tool.",
                    "tool_calls":[{
                        "id":"call_1",
                        "type":"function",
                        "function":{"name":"weather","arguments":"{\"city\":\"Paris\"}"}
                    }]
                },
                "finish_reason":"tool_calls"
            }],
            "usage":{
                "prompt_tokens":11,
                "completion_tokens":7,
                "prompt_tokens_details":{"cached_tokens":3},
                "completion_tokens_details":{"reasoning_tokens":2}
            }
        })))
        .expect("bridged response");
    let response_id = exchange
        .continuation_id_from_response(ProtocolOperation::Responses, &decoded_response)
        .expect("response identity")
        .expect("response id");
    let continuation = match exchange.bridge_continuation_state() {
        BridgeContinuationState::Ready(state) => state,
        other => panic!("buffered bridge continuation must be ready: {other:?}"),
    };
    let wrong_operation = decoded(
        &registry,
        ProtocolOperation::ChatCompletions,
        json!({
            "model":"public-model",
            "messages":[{"role":"user","content":"wrong exchange"}]
        }),
    )
    .await;
    let mut direct_exchange = registry
        .exchange(
            any2api_domain::ProtocolDialect::OpenAiChatCompletions,
            any2api_domain::ProtocolDialect::OpenAiChatCompletions,
            ProtocolOperation::ChatCompletions,
        )
        .expect("direct chat exchange");
    assert_eq!(
        direct_exchange
            .prepare_request(
                wrong_operation,
                "upstream-model",
                Some(continuation.clone())
            )
            .err()
            .expect("continuation cannot cross a direct exchange"),
        crate::ProtocolError::SessionBindingLost
    );
    let continuation_debug = format!("{continuation:?}");
    assert!(!continuation_debug.contains("What is the weather?"));
    assert!(!continuation_debug.contains("I will check."));
    let egress = exchange
        .encode_egress_response(decoded_response, "public-model")
        .expect("egress response");
    let response: Value = serde_json::from_slice(&egress.body).expect("Responses JSON");
    assert_eq!(response["id"], response_id);
    assert_eq!(response["output"][0]["type"], "reasoning");
    assert_eq!(response["output"][1]["type"], "message");
    assert_eq!(response["output"][2]["type"], "function_call");
    assert_eq!(response["output"][2]["call_id"], "call_1");
    assert_eq!(response["usage"]["input_tokens"], 11);
    assert_eq!(
        response["usage"]["output_tokens_details"]["reasoning_tokens"],
        2
    );

    let follow_up = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({
            "model":"public-model",
            "previous_response_id":response_id,
            "input":[{
                "type":"function_call_output",
                "call_id":"call_1",
                "output":"Sunny"
            }]
        }),
    )
    .await;
    let mut follow_up_exchange = bridged_exchange(&registry, ProtocolOperation::Responses);
    let prepared = follow_up_exchange
        .prepare_request(follow_up, "upstream-model", Some(continuation))
        .expect("follow-up request");
    let follow_up_body: Value =
        serde_json::from_slice(&prepared.request.body).expect("follow-up JSON");
    let messages = follow_up_body["messages"].as_array().expect("messages");
    assert_eq!(messages.len(), 4);
    assert_eq!(messages[2]["role"], "assistant");
    assert_eq!(messages[2]["tool_calls"][0]["id"], "call_1");
    assert_eq!(messages[3]["role"], "tool");
    assert_eq!(messages[3]["tool_call_id"], "call_1");
    assert_eq!(messages[3]["content"], "Sunny");
}
