use any2api_domain::ProtocolOperation;
use bytes::Bytes;
use serde_json::json;

use crate::{
    ProtocolError,
    api::{BridgeContinuationState, SseFrame},
};

use super::{bridged_exchange, chat_frame, decoded, registry};

#[tokio::test]
async fn streaming_bridge_rejects_tool_calls_without_a_valid_index() {
    let invalid_calls = [
        json!([
            {"id":"call_a","type":"function","function":{"name":"alpha","arguments":"{\"a\":1}"}},
            {"id":"call_b","type":"function","function":{"name":"beta","arguments":"{\"b\":2}"}}
        ]),
        json!([
            {"index":-1,"id":"call_a","type":"function","function":{"name":"alpha","arguments":"{}"}}
        ]),
        json!([
            {"index":"0","id":"call_a","type":"function","function":{"name":"alpha","arguments":"{}"}}
        ]),
    ];

    for calls in invalid_calls {
        let registry = registry();
        let request = decoded(
            &registry,
            ProtocolOperation::Responses,
            json!({"model":"public-model","input":"hello","stream":true}),
        )
        .await;
        let mut exchange = bridged_exchange(&registry, ProtocolOperation::Responses);
        exchange
            .prepare_request(&request, "upstream-model", None)
            .expect("stream request");

        let result = exchange.decode_upstream_event(chat_frame(json!({
            "id":"chatcmpl_invalid_tool_index",
            "model":"upstream-model",
            "choices":[{"index":0,"delta":{"tool_calls":calls}}]
        })));
        assert!(
            matches!(result, Err(ProtocolError::InvalidPayload(message)) if message.contains("index")),
            "invalid tool-call index must fail closed"
        );
        assert!(matches!(
            exchange.bridge_continuation_state(),
            BridgeContinuationState::Pending
        ));
    }
}

#[tokio::test]
async fn streaming_bridge_rejects_incomplete_tool_identity_at_finish_reason() {
    let registry = registry();
    let request = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({"model":"public-model","input":"hello","stream":true}),
    )
    .await;
    let mut exchange = bridged_exchange(&registry, ProtocolOperation::Responses);
    exchange
        .prepare_request(&request, "upstream-model", None)
        .expect("stream request");

    let result = exchange.decode_upstream_event(chat_frame(json!({
        "id":"chatcmpl_incomplete_tool",
        "model":"upstream-model",
        "choices":[{
            "index":0,
            "delta":{"tool_calls":[{
                "index":0,
                "type":"function",
                "function":{"name":"weather","arguments":"{}"}
            }]},
            "finish_reason":"tool_calls"
        }]
    })));

    assert!(
        matches!(result, Err(ProtocolError::InvalidPayload(message)) if message.contains("identity is incomplete")),
        "a terminal chunk must fail before returning any synthesized events"
    );
    assert!(matches!(
        exchange.bridge_continuation_state(),
        BridgeContinuationState::Pending
    ));
}

#[tokio::test]
async fn streaming_bridge_rejects_incomplete_tool_identity_at_done_without_partial_done() {
    let registry = registry();
    let request = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({"model":"public-model","input":"hello","stream":true}),
    )
    .await;
    let mut exchange = bridged_exchange(&registry, ProtocolOperation::Responses);
    exchange
        .prepare_request(&request, "upstream-model", None)
        .expect("stream request");

    let initial = exchange
        .decode_upstream_event(chat_frame(json!({
            "id":"chatcmpl_mixed_tools",
            "model":"upstream-model",
            "choices":[{"index":0,"delta":{"tool_calls":[
                {
                    "index":0,"id":"call_valid","type":"function",
                    "function":{"name":"valid","arguments":"{}"}
                },
                {
                    "index":1,"type":"function",
                    "function":{"name":"missing_id","arguments":"{}"}
                }
            ]}}]
        })))
        .expect("non-terminal fragments can await identity completion");
    let initial = initial
        .iter()
        .map(|event| String::from_utf8_lossy(event.bytes()))
        .collect::<String>();
    assert!(initial.contains("response.output_item.added"));
    assert!(!initial.contains("response.output_item.done"));

    let result = exchange.decode_upstream_event(SseFrame(Bytes::from_static(b"data: [DONE]\n\n")));
    assert!(
        matches!(result, Err(ProtocolError::InvalidPayload(message)) if message.contains("identity is incomplete")),
        "DONE must reject the entire completion instead of returning a valid tool's done event"
    );
    assert!(matches!(
        exchange.bridge_continuation_state(),
        BridgeContinuationState::Pending
    ));
}
