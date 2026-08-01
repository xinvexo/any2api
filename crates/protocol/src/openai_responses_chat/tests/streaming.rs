use any2api_domain::ProtocolOperation;
use bytes::Bytes;
use serde_json::{Value, json};

use crate::{
    ProtocolError,
    api::{
        BridgeContinuationState, MAX_BRIDGE_CONTINUATION_STATE_BYTES, SseFrame, StreamTermination,
    },
};

use super::{bridged_exchange, chat_frame, decoded, registry};

#[tokio::test]
async fn streaming_bridge_emits_responses_events_tools_and_usage() {
    let registry = registry();
    let request = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({"model":"public-model","input":"hello","stream":true}),
    )
    .await;
    let mut exchange = bridged_exchange(&registry, ProtocolOperation::Responses);
    let prepared = exchange
        .prepare_request(request, "upstream-model", None)
        .expect("stream request");
    assert!(matches!(
        exchange.bridge_continuation_state(),
        BridgeContinuationState::Pending
    ));
    let upstream_request: Value =
        serde_json::from_slice(&prepared.request.body).expect("upstream request JSON");
    assert_eq!(upstream_request["stream"], true);
    assert_eq!(upstream_request["stream_options"]["include_usage"], true);

    let frames = [
        chat_frame(json!({
            "id":"chatcmpl_stream","model":"upstream-model",
            "choices":[{"index":0,"delta":{"role":"assistant","content":"Hello "}}]
        })),
        chat_frame(json!({
            "id":"chatcmpl_stream","model":"upstream-model",
            "choices":[{"index":0,"delta":{"tool_calls":[{
                "index":0,"id":"call_1","type":"function",
                "function":{"name":"weather","arguments":"{\"city\":\""}
            }]}}]
        })),
        chat_frame(json!({
            "id":"chatcmpl_stream","model":"upstream-model",
            "choices":[{"index":0,"delta":{"content":"world","tool_calls":[{
                "index":0,"function":{"arguments":"Paris\"}"}
            }]},"finish_reason":"tool_calls"}]
        })),
        chat_frame(json!({
            "id":"chatcmpl_stream","model":"upstream-model","choices":[],
            "usage":{
                "prompt_tokens":4,"completion_tokens":3,
                "prompt_tokens_details":{"cached_tokens":1},
                "completion_tokens_details":{"reasoning_tokens":0}
            }
        })),
        SseFrame(Bytes::from_static(b"data: [DONE]\n\n")),
    ];
    let mut output = String::new();
    let mut terminal_usage = None;
    for frame in frames {
        for event in exchange
            .decode_upstream_event(frame)
            .expect("bridged stream event")
        {
            if event
                .bytes()
                .windows(18)
                .any(|window| window == b"response.completed")
            {
                assert_eq!(event.termination(), StreamTermination::Completed);
                terminal_usage = Some(event.telemetry().token_usage);
            }
            let frame = exchange
                .encode_egress_event(event, "public-model")
                .expect("egress event");
            output.push_str(std::str::from_utf8(&frame.0).expect("UTF-8 SSE"));
        }
    }
    assert!(
        exchange
            .finish_upstream_events()
            .expect("finish stream")
            .is_empty()
    );
    assert!(output.contains("response.output_text.delta"));
    assert!(output.contains("response.function_call_arguments.done"));
    assert!(output.contains("Paris"));
    assert!(output.contains(r#""model":"public-model""#));
    assert!(output.contains(r#""input_tokens":4"#));
    let usage = terminal_usage.expect("terminal usage");
    assert_eq!(usage.input_tokens(), Some(4));
    assert_eq!(usage.output_tokens(), Some(3));
    assert_eq!(usage.cache_read_tokens(), Some(1));
    assert!(matches!(
        exchange.bridge_continuation_state(),
        BridgeContinuationState::Ready(_)
    ));
}

#[tokio::test]
async fn streaming_bridge_marks_incomplete_as_a_successful_terminal() {
    let registry = registry();
    let request = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({"model":"public-model","input":"hello","stream":true}),
    )
    .await;
    let mut exchange = bridged_exchange(&registry, ProtocolOperation::Responses);
    exchange
        .prepare_request(request, "upstream-model", None)
        .expect("stream request");

    exchange
        .decode_upstream_event(chat_frame(json!({
            "id":"chatcmpl_incomplete","model":"upstream-model",
            "choices":[{"index":0,"delta":{"content":"partial"},"finish_reason":"length"}]
        })))
        .expect("partial event");
    let terminal = exchange
        .decode_upstream_event(SseFrame(Bytes::from_static(b"data: [DONE]\n\n")))
        .expect("terminal event")
        .into_iter()
        .find(|event| event.is_terminal())
        .expect("incomplete terminal");

    assert!(String::from_utf8_lossy(terminal.bytes()).contains("response.incomplete"));
    assert_eq!(terminal.termination(), StreamTermination::Completed);
}

#[tokio::test]
async fn streaming_bridge_aborts_when_incremental_state_exceeds_the_hard_limit() {
    let registry = registry();
    let request = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({"model":"public-model","input":"hello","stream":true}),
    )
    .await;
    let mut exchange = bridged_exchange(&registry, ProtocolOperation::Responses);
    exchange
        .prepare_request(request, "upstream-model", None)
        .expect("stream request");

    let oversized = chat_frame(json!({
        "id":"chatcmpl_oversized",
        "model":"upstream-model",
        "choices":[{"index":0,"delta":{"content":"x".repeat(MAX_BRIDGE_CONTINUATION_STATE_BYTES)}}]
    }));
    assert!(matches!(
        exchange.decode_upstream_event(oversized),
        Err(ProtocolError::ContinuationTooLarge { .. })
    ));
    assert!(matches!(
        exchange.bridge_continuation_state(),
        BridgeContinuationState::Pending
    ));
}
