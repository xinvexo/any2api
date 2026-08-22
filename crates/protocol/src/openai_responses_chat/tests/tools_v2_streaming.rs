use any2api_domain::{OpenAiChatCompletionsProfile, ProtocolOperation, ProtocolTargetProfile};
use bytes::Bytes;
use serde_json::{Value, json};

use crate::api::{SseEventPayload, SseFrame};

use super::{bridged_exchange, chat_frame, decoded, registry};

#[tokio::test]
async fn native_custom_stream_preserves_identity_and_input_deltas() {
    let registry = registry();
    let request = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({
            "model":"public","input":"write SQL","stream":true,
            "tools":[{"type":"custom","name":"sql","format":{"type":"text"}}]
        }),
    )
    .await;
    let profile = OpenAiChatCompletionsProfile::CURRENT_OPENAI;
    let mut exchange = bridged_exchange(&registry, ProtocolOperation::Responses);
    exchange
        .prepare_request_with_target_profile(
            &request,
            "upstream",
            Some(ProtocolTargetProfile::OpenAiChatCompletions(profile)),
            None,
        )
        .expect("native custom stream request");

    let frames = [
        chat_frame(json!({
            "model":"upstream","choices":[{"index":0,"delta":{"role":"assistant",
                "tool_calls":[{"index":0,"id":"call_sql","type":"custom",
                    "custom":{"name":"sql","input":"SELECT "}}]}}]
        })),
        chat_frame(json!({
            "model":"upstream","choices":[{"index":0,"delta":{"tool_calls":[{
                "index":0,"id":"call_sql","type":"custom",
                "custom":{"name":"sql","input":"1"}}]},"finish_reason":"tool_calls"}]
        })),
        SseFrame(Bytes::from_static(b"data: [DONE]\n\n")),
    ];
    let mut output = String::new();
    for frame in frames {
        for event in exchange
            .decode_upstream_event(frame)
            .expect("native custom stream event")
        {
            output.push_str(&String::from_utf8_lossy(event.bytes()));
        }
    }
    assert!(output.contains("response.custom_tool_call_input.delta"));
    assert!(output.contains("response.custom_tool_call_input.done"));
    assert!(output.contains(r#""type":"custom_tool_call""#));
    assert!(output.contains(r#""input":"SELECT 1""#));
    assert!(!output.contains("call_sqlcall_sql"));
    assert!(!output.contains("sqlsql"));
}

#[tokio::test]
async fn namespace_and_tool_search_streams_restore_responses_items() {
    let registry = registry();
    for (tool, expected_type, expected_field, expected_value) in [
        (
            json!({"type":"namespace","name":"calendar","description":"Calendar","tools":[{
                "type":"function","name":"create","parameters":{"type":"object"}
            }]}),
            "function_call",
            "namespace",
            "calendar",
        ),
        (
            json!({"type":"tool_search","execution":"client","description":"Search",
                "parameters":{"type":"object","properties":{"query":{"type":"string"}}}}),
            "tool_search_call",
            "execution",
            "client",
        ),
    ] {
        let request = decoded(
            &registry,
            ProtocolOperation::Responses,
            json!({"model":"public","input":"use tool","stream":true,"tools":[tool]}),
        )
        .await;
        let mut exchange = bridged_exchange(&registry, ProtocolOperation::Responses);
        let prepared = exchange
            .prepare_request(&request, "upstream", None)
            .expect("stream request");
        let upstream: Value =
            serde_json::from_slice(&prepared.request.body).expect("Chat stream request");
        let projected = upstream["tools"][0]["function"]["name"]
            .as_str()
            .expect("projected name")
            .to_owned();
        let arguments = if expected_type == "tool_search_call" {
            "{\"query\":\"calendar\"}"
        } else {
            "{}"
        };
        let frames = [
            chat_frame(json!({
                "model":"upstream","choices":[{"index":0,"delta":{"role":"assistant","tool_calls":[{
                    "index":0,"id":"call_1","type":"function",
                    "function":{"name":projected,"arguments":arguments}
                }]},"finish_reason":"tool_calls"}]
            })),
            SseFrame(Bytes::from_static(b"data: [DONE]\n\n")),
        ];
        let mut events = Vec::new();
        for frame in frames {
            for event in exchange
                .decode_upstream_event(frame)
                .expect("restored stream event")
            {
                let SseEventPayload::Json(data) = event.payload() else {
                    panic!("bridged event must contain JSON");
                };
                let payload = data.to_value().expect("event value");
                events.push(payload);
            }
        }
        let item = events
            .iter()
            .find(|event| {
                event["type"] == "response.output_item.done"
                    && event["item"]["type"] == expected_type
            })
            .map(|event| &event["item"])
            .expect("restored output item");
        assert_eq!(item[expected_field], expected_value);
    }
}
