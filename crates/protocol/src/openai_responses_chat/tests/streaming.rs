use any2api_domain::{ProtocolOperation, RetrySafety};
use bytes::Bytes;
use serde_json::{Value, json};

use crate::{
    ProtocolError,
    api::{
        BridgeContinuationState, MAX_BRIDGE_CONTINUATION_STATE_BYTES, ProtocolRetryDelayBasis,
        SseFrame, StreamTermination,
    },
};

use super::{bridged_exchange, chat_frame, decoded, registry};

#[tokio::test]
async fn streaming_bridge_emits_responses_events_tools_and_usage() {
    let registry = registry();
    let request = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({"model":"public-model","input":"hello","stream":true,"tools":[{
            "type":"function","name":"weather","parameters":{"type":"object"}
        }]}),
    )
    .await;
    let mut exchange = bridged_exchange(&registry, ProtocolOperation::Responses);
    let prepared = exchange
        .prepare_request(&request, "upstream-model", None)
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
            "id":"chatcmpl_stream","model":"upstream-model",
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
    assert_contiguous_sequence_numbers(&output);
    let events = response_events(&output);
    let message_id = events
        .iter()
        .find(|event| {
            event["type"] == "response.output_item.added" && event["item"]["type"] == "message"
        })
        .and_then(|event| event["item"]["id"].as_str())
        .expect("streamed message item id");
    let tool_id = events
        .iter()
        .find(|event| {
            event["type"] == "response.output_item.added"
                && event["item"]["type"] == "function_call"
        })
        .and_then(|event| event["item"]["id"].as_str())
        .expect("streamed function-call item id");
    assert!(message_id.starts_with("msg_"));
    assert!(tool_id.starts_with("fc_"));
    for event in &events {
        let Some(item_id) = event["item_id"].as_str() else {
            continue;
        };
        if event["output_index"] == 0 {
            assert_eq!(item_id, message_id);
        } else if event["output_index"] == 1 {
            assert_eq!(item_id, tool_id);
        }
    }
    let completed = events
        .iter()
        .find(|event| event["type"] == "response.completed")
        .expect("completed event");
    assert_eq!(completed["response"]["output"][0]["id"], message_id);
    assert_eq!(completed["response"]["output"][1]["id"], tool_id);
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
async fn streaming_reasoning_closes_text_part_and_item_in_order() {
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
    let frames = [
        chat_frame(json!({
            "id":"chatcmpl_reasoning","model":"upstream-model",
            "choices":[{"index":0,"delta":{"role":"assistant","reasoning_content":"plan "}}]
        })),
        chat_frame(json!({
            "id":"chatcmpl_reasoning","model":"upstream-model",
            "choices":[{"index":0,"delta":{"reasoning_content":"answer"},"finish_reason":"stop"}]
        })),
        SseFrame(Bytes::from_static(b"data: [DONE]\n\n")),
    ];
    let mut events = Vec::new();
    for frame in frames {
        for event in exchange
            .decode_upstream_event(frame)
            .expect("bridged reasoning event")
        {
            let frame = exchange
                .encode_egress_event(event, "public-model")
                .expect("egress reasoning event");
            let text = std::str::from_utf8(&frame.0).expect("UTF-8 SSE");
            let data = text
                .lines()
                .find_map(|line| line.strip_prefix("data: "))
                .expect("SSE data");
            events.push(serde_json::from_str::<Value>(data).expect("Responses event JSON"));
        }
    }

    let lifecycle = events
        .iter()
        .filter_map(|event| event["type"].as_str())
        .filter(|kind| {
            kind.starts_with("response.reasoning_summary_")
                || matches!(
                    *kind,
                    "response.output_item.added" | "response.output_item.done"
                )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        lifecycle,
        [
            "response.output_item.added",
            "response.reasoning_summary_part.added",
            "response.reasoning_summary_text.delta",
            "response.reasoning_summary_text.delta",
            "response.reasoning_summary_text.done",
            "response.reasoning_summary_part.done",
            "response.output_item.done",
        ]
    );
    let part_done = events
        .iter()
        .find(|event| event["type"] == "response.reasoning_summary_part.done")
        .expect("reasoning part done event");
    assert_eq!(part_done["summary_index"], 0);
    assert_eq!(
        part_done["part"],
        json!({"type":"summary_text","text":"plan answer"})
    );
    let sequence_numbers = events
        .iter()
        .map(|event| event["sequence_number"].as_u64().expect("sequence number"))
        .collect::<Vec<_>>();
    assert_eq!(
        sequence_numbers,
        (0..sequence_numbers.len() as u64).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn streaming_bridge_maps_choice_less_error_chunks_to_responses_errors() {
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

    let events = exchange
        .decode_upstream_event(chat_frame(json!({
            "error":{
                "message":"upstream overloaded",
                "type":"server_error",
                "param":"tools",
                "code":"server_is_overloaded"
            }
        })))
        .expect("error chunk maps to Responses");
    assert_eq!(events.len(), 1);
    let event = &events[0];
    assert_eq!(event.termination(), StreamTermination::Failed);
    let evidence = event
        .upstream_failure()
        .expect("bridged failure preserves upstream evidence");
    assert_eq!(
        evidence.retry_safety_override(),
        Some(RetrySafety::RejectedBeforeExecution)
    );
    assert_eq!(
        evidence.retry_delay_basis(),
        ProtocolRetryDelayBasis::RequestAttempts
    );
    assert!(String::from_utf8_lossy(evidence.raw_json()).contains("server_is_overloaded"));
    let text = std::str::from_utf8(event.bytes()).expect("Responses error UTF-8");
    assert!(text.contains(r#""type":"error""#));
    assert!(text.contains(r#""code":"server_is_overloaded""#));
    assert!(text.contains(r#""message":"upstream overloaded""#));
    assert!(text.contains(r#""param":"tools""#));
    assert!(text.contains(r#""sequence_number":0"#));
    assert!(matches!(
        exchange.bridge_continuation_state(),
        BridgeContinuationState::Pending
    ));
    assert!(
        exchange
            .finish_upstream_events()
            .expect("failed stream stays finished")
            .is_empty()
    );
}

#[tokio::test]
async fn streaming_bridge_marks_incomplete_as_a_successful_terminal() {
    for (finish_reason, incomplete_reason) in [
        ("length", "max_output_tokens"),
        ("content_filter", "content_filter"),
    ] {
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

        exchange
            .decode_upstream_event(chat_frame(json!({
                "id":"chatcmpl_incomplete","model":"upstream-model",
                "choices":[{
                    "index":0,
                    "delta":{"role":"assistant","content":"partial"},
                    "finish_reason":finish_reason
                }]
            })))
            .expect("partial event");
        let terminal = exchange
            .decode_upstream_event(SseFrame(Bytes::from_static(b"data: [DONE]\n\n")))
            .expect("terminal event")
            .into_iter()
            .find(|event| event.is_terminal())
            .expect("incomplete terminal");
        let terminal_json = response_events(&String::from_utf8_lossy(terminal.bytes()))
            .into_iter()
            .next()
            .expect("terminal Responses event");

        assert_eq!(terminal_json["type"], "response.incomplete");
        assert_eq!(terminal_json["response"]["status"], "incomplete");
        assert_eq!(
            terminal_json["response"]["incomplete_details"]["reason"],
            incomplete_reason
        );
        assert_eq!(terminal.termination(), StreamTermination::Completed);
    }
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
        .prepare_request(&request, "upstream-model", None)
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

fn assert_contiguous_sequence_numbers(stream: &str) {
    let sequence_numbers = response_events(stream)
        .into_iter()
        .map(|event| {
            event["sequence_number"]
                .as_u64()
                .expect("Responses SSE sequence_number")
        })
        .collect::<Vec<_>>();
    assert!(!sequence_numbers.is_empty());
    assert_eq!(
        sequence_numbers,
        (0..sequence_numbers.len() as u64).collect::<Vec<_>>()
    );
}

fn response_events(stream: &str) -> Vec<Value> {
    stream
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .map(|data| serde_json::from_str::<Value>(data).expect("Responses SSE JSON"))
        .collect()
}
