use any2api_domain::ProtocolOperation;
use serde_json::{Value, json};

use crate::api::{BridgeContinuationState, OpenAiChatCompletionsProfile, ProtocolTargetProfile};

use super::{bridged_exchange, decoded, registry, upstream_response};

#[tokio::test]
async fn baseline_custom_tool_uses_a_reversible_function_envelope_across_continuation() {
    let registry = registry();
    let source = json!({
        "model":"public",
        "input":"Run the script",
        "temperature":0.2,
        "store":false,
        "tools":[{
            "type":"custom","name":"shell","description":"Run a shell script",
            "format":{"type":"text"}
        }],
        "tool_choice":{"type":"custom","name":"shell"}
    });
    let request = decoded(&registry, ProtocolOperation::Responses, source).await;
    let mut exchange = bridged_exchange(&registry, ProtocolOperation::Responses);
    let prepared = exchange
        .prepare_request(&request, "upstream-model", None)
        .expect("custom tool envelope request");
    let upstream: Value = serde_json::from_slice(&prepared.request.body).expect("Chat request");
    let projected_name = upstream["tools"][0]["function"]["name"]
        .as_str()
        .expect("projected custom name")
        .to_owned();
    assert_eq!(upstream["tools"][0]["type"], "function");
    assert_ne!(projected_name, "shell");
    assert_eq!(
        upstream["tools"][0]["function"]["parameters"]["required"],
        json!(["input"])
    );
    assert_eq!(upstream["tool_choice"]["function"]["name"], projected_name);

    let decoded_response = exchange
        .decode_upstream_response(upstream_response(json!({
            "choices":[{
                "index":0,
                "message":{"role":"assistant","content":null,"tool_calls":[{
                    "id":"call_shell","type":"function","function":{
                        "name":projected_name,"arguments":"{\"input\":\"echo hello\"}"
                    }
                }]},
                "finish_reason":"tool_calls"
            }]
        })))
        .expect("custom envelope response");
    let response_id = exchange
        .continuation_id_from_response(ProtocolOperation::Responses, &decoded_response)
        .expect("response identity")
        .expect("response id");
    let continuation = match exchange.bridge_continuation_state() {
        BridgeContinuationState::Ready(state) => state,
        other => panic!("custom call continuation must be ready: {other:?}"),
    };
    let egress = exchange
        .encode_egress_response(decoded_response, "public")
        .expect("Responses response");
    let response: Value = serde_json::from_slice(&egress.body).expect("Responses JSON");
    assert!(
        response["created_at"]
            .as_u64()
            .is_some_and(|value| value > 0)
    );
    assert_eq!(response["model"], "public");
    assert_eq!(response["usage"], Value::Null);
    assert_eq!(response["temperature"], 0.2);
    assert_eq!(response["store"], false);
    assert_eq!(response["output"][0]["type"], "custom_tool_call");
    assert_eq!(response["output"][0]["name"], "shell");
    assert_eq!(response["output"][0]["input"], "echo hello");
    assert!(
        response["output"][0]["id"]
            .as_str()
            .is_some_and(|id| id.starts_with("ctc_"))
    );

    let follow_up = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({
            "model":"public","previous_response_id":response_id,
            "input":[{"type":"custom_tool_call_output","call_id":"call_shell",
                "name":"shell","output":"hello"}]
        }),
    )
    .await;
    let prepared = bridged_exchange(&registry, ProtocolOperation::Responses)
        .prepare_request(&follow_up, "upstream-model", Some(continuation))
        .expect("custom output continuation");
    let upstream: Value = serde_json::from_slice(&prepared.request.body).expect("follow-up Chat");
    let messages = upstream["messages"].as_array().expect("messages");
    assert_eq!(messages.last().expect("tool message")["role"], "tool");
    assert_eq!(
        messages.last().expect("tool message")["tool_call_id"],
        "call_shell"
    );
    assert_eq!(messages.last().expect("tool message")["content"], "hello");
}

#[tokio::test]
async fn current_openai_profile_preserves_native_custom_and_multimodal_chat_fields() {
    let registry = registry();
    let request = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({
            "model":"public","instructions":"Use the grammar","max_output_tokens":128,
            "input":[{"type":"message","role":"user","content":[
                {"type":"input_text","text":"Read these inputs"},
                {"type":"input_audio","input_audio":{"data":"AA==","format":"wav"}},
                {"type":"input_file","file_data":"data:text/plain;base64,QQ==","filename":"a.txt"}
            ]}],
            "tools":[{"type":"custom","name":"sql","description":"Produce SQL",
                "format":{"type":"grammar","syntax":"lark","definition":"start: SELECT"}}],
            "tool_choice":{"type":"custom","name":"sql"}
        }),
    )
    .await;
    let profile = OpenAiChatCompletionsProfile::CURRENT_OPENAI;
    let mut exchange = bridged_exchange(&registry, ProtocolOperation::Responses);
    let prepared = exchange
        .prepare_request_with_target_profile(
            &request,
            "upstream-model",
            Some(ProtocolTargetProfile::OpenAiChatCompletions(profile)),
            None,
        )
        .expect("native custom request");
    let upstream: Value = serde_json::from_slice(&prepared.request.body).expect("Chat request");
    assert_eq!(upstream["messages"][0]["role"], "developer");
    assert_eq!(upstream["max_completion_tokens"], 128);
    assert!(upstream.get("max_tokens").is_none());
    assert_eq!(upstream["messages"][1]["content"][1]["type"], "input_audio");
    assert_eq!(upstream["messages"][1]["content"][2]["type"], "file");
    assert_eq!(upstream["tools"][0]["type"], "custom");
    assert_eq!(upstream["tools"][0]["custom"]["name"], "sql");
    assert_eq!(upstream["tool_choice"]["type"], "custom");

    let decoded_response = exchange
        .decode_upstream_response(upstream_response(json!({
            "created":42,"model":"upstream-model",
            "choices":[{"index":0,"message":{"role":"assistant","content":null,
                "tool_calls":[{"id":"call_sql","type":"custom",
                    "custom":{"name":"sql","input":"SELECT 1"}}]},
                "finish_reason":"tool_calls"}]
        })))
        .expect("native custom response");
    let egress = exchange
        .encode_egress_response(decoded_response, "public")
        .expect("Responses response");
    let response: Value = serde_json::from_slice(&egress.body).expect("Responses JSON");
    assert_eq!(response["output"][0]["type"], "custom_tool_call");
    assert_eq!(response["output"][0]["input"], "SELECT 1");
}

#[tokio::test]
async fn namespace_function_names_are_bounded_and_restored() {
    let registry = registry();
    let request = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({
            "model":"public","input":"Create the event",
            "tools":[{"type":"namespace","name":"calendar","description":"Calendar tools",
                "tools":[{"type":"function","name":"create_event","description":"Create",
                    "parameters":{"type":"object","properties":{}}}]}],
            "tool_choice":{"type":"function","namespace":"calendar","name":"create_event"}
        }),
    )
    .await;
    let mut exchange = bridged_exchange(&registry, ProtocolOperation::Responses);
    let prepared = exchange
        .prepare_request(&request, "upstream", None)
        .expect("namespace request");
    let upstream: Value = serde_json::from_slice(&prepared.request.body).expect("Chat request");
    let projected = upstream["tools"][0]["function"]["name"]
        .as_str()
        .expect("flattened name");
    assert!(projected.len() <= 64);
    assert_ne!(projected, "create_event");
    assert_eq!(upstream["tool_choice"]["function"]["name"], projected);

    let projected = projected.to_owned();
    let decoded_response = exchange
        .decode_upstream_response(upstream_response(json!({
            "choices":[{"index":0,"message":{"role":"assistant","content":null,
                "tool_calls":[{"id":"call_calendar","type":"function","function":{
                    "name":projected,"arguments":"{}"}}]},"finish_reason":"tool_calls"}]
        })))
        .expect("namespace response");
    let egress = exchange
        .encode_egress_response(decoded_response, "public")
        .expect("Responses response");
    let response: Value = serde_json::from_slice(&egress.body).expect("Responses JSON");
    assert_eq!(response["output"][0]["type"], "function_call");
    assert_eq!(response["output"][0]["namespace"], "calendar");
    assert_eq!(response["output"][0]["name"], "create_event");
}

#[tokio::test]
async fn client_tool_search_round_trips_call_and_output() {
    let registry = registry();
    let request = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({
            "model":"public","input":"Find a calendar tool",
            "tools":[
                {"type":"function","name":"calendar_create","defer_loading":true,
                    "parameters":{"type":"object","properties":{}}},
                {"type":"tool_search","execution":"client",
                    "description":"Search available tools",
                    "parameters":{"type":"object","properties":{"query":{"type":"string"}}}}
            ],
            "tool_choice":{"type":"tool_search"}
        }),
    )
    .await;
    let mut exchange = bridged_exchange(&registry, ProtocolOperation::Responses);
    let prepared = exchange
        .prepare_request(&request, "upstream", None)
        .expect("tool_search request");
    let upstream: Value = serde_json::from_slice(&prepared.request.body).expect("Chat request");
    let projected = upstream["tools"][0]["function"]["name"]
        .as_str()
        .expect("tool_search projected name")
        .to_owned();
    assert_eq!(upstream["tools"].as_array().map(Vec::len), Some(1));

    let decoded_response = exchange
        .decode_upstream_response(upstream_response(json!({
            "choices":[{"index":0,"message":{"role":"assistant","content":null,
                "tool_calls":[{"id":"search_1","type":"function","function":{
                    "name":projected,"arguments":"{\"query\":\"calendar\",\"limit\":1}"}}]},
                "finish_reason":"tool_calls"}]
        })))
        .expect("tool_search response");
    let response_id = exchange
        .continuation_id_from_response(ProtocolOperation::Responses, &decoded_response)
        .expect("response identity")
        .expect("response id");
    let continuation = match exchange.bridge_continuation_state() {
        BridgeContinuationState::Ready(state) => state,
        other => panic!("tool_search continuation must be ready: {other:?}"),
    };
    let egress = exchange
        .encode_egress_response(decoded_response, "public")
        .expect("Responses response");
    let response: Value = serde_json::from_slice(&egress.body).expect("Responses JSON");
    assert_eq!(response["output"][0]["type"], "tool_search_call");
    assert_eq!(response["output"][0]["execution"], "client");
    assert_eq!(response["output"][0]["arguments"]["query"], "calendar");
    assert!(
        response["output"][0]["id"]
            .as_str()
            .is_some_and(|id| id.starts_with("tsc_"))
    );

    let follow_up = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({
            "model":"public","previous_response_id":response_id,
            "tools":[
                {"type":"function","name":"calendar_create","defer_loading":true,
                    "parameters":{"type":"object","properties":{}}},
                {"type":"tool_search","execution":"client","description":"Search available tools",
                    "parameters":{"type":"object","properties":{"query":{"type":"string"}}}}
            ],
            "input":[{"type":"tool_search_output","call_id":"search_1",
                "status":"completed","execution":"client","tools":[{
                    "type":"function","name":"calendar_create","defer_loading":true,
                    "parameters":{"type":"object"}
                }]}]
        }),
    )
    .await;
    let prepared = bridged_exchange(&registry, ProtocolOperation::Responses)
        .prepare_request(&follow_up, "upstream", Some(continuation))
        .expect("tool_search output continuation");
    let upstream: Value = serde_json::from_slice(&prepared.request.body).expect("follow-up Chat");
    assert_eq!(upstream["tools"].as_array().map(Vec::len), Some(2));
    assert!(
        upstream["tools"]
            .as_array()
            .expect("activated tools")
            .iter()
            .any(|tool| tool["function"]["name"] == "calendar_create")
    );
    let content = upstream["messages"]
        .as_array()
        .and_then(|messages| messages.last())
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .expect("tool_search output content");
    let content: Value = serde_json::from_str(content).expect("structured tool_search output");
    assert_eq!(content["execution"], "client");
    assert_eq!(content["tools"][0]["name"], "calendar_create");
}
