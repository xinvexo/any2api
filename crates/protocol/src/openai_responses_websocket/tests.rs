use bytes::Bytes;
use http::{HeaderMap, HeaderValue, StatusCode};
use serde_json::{Value, json};

use super::{
    ResponsesWsConversation, ResponsesWsIngress, ResponsesWsObservation, WsResolveError,
    WsResponseOutcome, classify_egress_frame, previous_response_not_found_event,
    warmup_completed_event, warmup_created_event, wrapped_error_event,
};
use crate::api::SseFrame;

const MAX_BODY: usize = 32 * 1024 * 1024;
const MAX_STATE: usize = 32 * 1024 * 1024;

fn request_text(previous: Option<&str>, generate: Option<bool>, input: Value) -> String {
    let mut request = json!({
        "type": "response.create",
        "model": "gpt-test",
        "instructions": "inst",
        "stream": true,
        "store": false,
        "tool_choice": "auto",
        "input": input,
        "client_metadata": {
            "session_id": "session-1",
            "thread_id": "thread-1",
            "x-codex-turn-state": "turn-state-1",
            "x-codex-ws-stream-request-start-ms": "123",
        },
    });
    if let Some(previous) = previous {
        request["previous_response_id"] = json!(previous);
    }
    if let Some(generate) = generate {
        request["generate"] = json!(generate);
    }
    request.to_string()
}

fn body_json(conversation: &ResponsesWsConversation, ingress: &ResponsesWsIngress) -> Value {
    let resolved = conversation
        .resolve(ingress, MAX_BODY)
        .expect("resolved request");
    serde_json::from_slice(&resolved.body()).expect("restored body JSON")
}

#[test]
fn parse_splits_transport_controls_from_the_request_payload() {
    let ingress = ResponsesWsIngress::parse(&request_text(
        Some("resp_prev"),
        Some(false),
        json!([{"type": "message", "role": "user"}]),
    ))
    .expect("parsed request");

    assert_eq!(ingress.previous_response_id(), Some("resp_prev"));
    assert!(ingress.is_warmup());
    assert_eq!(ingress.turn_state(), Some("turn-state-1"));
    assert_eq!(ingress.session_id(), Some("session-1"));
    assert_eq!(ingress.thread_id(), Some("thread-1"));

    let conversation = ResponsesWsConversation::new(MAX_STATE);
    let ingress = ResponsesWsIngress::parse(&request_text(None, None, json!([]))).expect("parsed");
    let body = body_json(&conversation, &ingress);
    assert!(body.get("type").is_none());
    assert!(body.get("previous_response_id").is_none());
    assert!(body.get("generate").is_none());
    let metadata = body["client_metadata"].as_object().expect("metadata kept");
    assert_eq!(metadata.get("session_id"), Some(&json!("session-1")));
    assert!(!metadata.contains_key("x-codex-turn-state"));
    assert!(!metadata.contains_key("x-codex-ws-stream-request-start-ms"));
    assert_eq!(body["model"], "gpt-test");
    assert_eq!(body["input"], json!([]));
}

#[test]
fn parse_drops_client_metadata_when_only_transport_keys_remain() {
    let text = json!({
        "type": "response.create",
        "model": "gpt-test",
        "stream": true,
        "input": [],
        "client_metadata": {"x-codex-turn-state": "turn"},
    })
    .to_string();
    let ingress = ResponsesWsIngress::parse(&text).expect("parsed");
    assert_eq!(ingress.turn_state(), Some("turn"));
    let body = body_json(&ResponsesWsConversation::new(MAX_STATE), &ingress);
    assert!(body.get("client_metadata").is_none());
}

#[test]
fn parse_rejects_invalid_transport_fields() {
    for text in [
        json!({"model": "m", "stream": true, "input": []}).to_string(),
        json!({"type": "response.cancel", "stream": true}).to_string(),
        json!({"type": "response.create", "model": "m", "input": []}).to_string(),
        json!({"type": "response.create", "stream": false, "input": []}).to_string(),
        json!({"type": "response.create", "stream": true, "previous_response_id": 5}).to_string(),
        json!({"type": "response.create", "stream": true, "generate": "no"}).to_string(),
        json!({"type": "response.create", "stream": true, "input": {}}).to_string(),
        json!({"type": "response.create", "stream": true, "client_metadata": []}).to_string(),
        "not json".to_owned(),
        "[]".to_owned(),
    ] {
        assert!(
            ResponsesWsIngress::parse(&text).is_err(),
            "accepted: {text}"
        );
    }
}

#[test]
fn incremental_requests_extend_the_completed_baseline() {
    let mut conversation = ResponsesWsConversation::new(MAX_STATE);
    let first = ResponsesWsIngress::parse(&request_text(
        None,
        None,
        json!([{"role": "user", "index": 1}]),
    ))
    .expect("first request");
    let resolved = conversation.resolve(&first, MAX_BODY).expect("resolved");
    conversation.finish_exchange(
        resolved,
        WsResponseOutcome {
            response_id: Some("resp_1".to_owned()),
            output_items: vec![json!({"type": "message", "role": "assistant", "index": 2})],
            output_bytes: 64,
            completed: true,
        },
    );

    let second = ResponsesWsIngress::parse(&request_text(
        Some("resp_1"),
        None,
        json!([{"role": "user", "index": 3}]),
    ))
    .expect("second request");
    let body = body_json(&conversation, &second);
    assert_eq!(
        body["input"],
        json!([
            {"role": "user", "index": 1},
            {"type": "message", "role": "assistant", "index": 2},
            {"role": "user", "index": 3},
        ])
    );

    let unknown = ResponsesWsIngress::parse(&request_text(Some("resp_other"), None, json!([])))
        .expect("unknown continuation");
    assert_eq!(
        conversation.resolve(&unknown, MAX_BODY).err(),
        Some(WsResolveError::PreviousResponseNotFound)
    );
}

#[test]
fn non_completed_outcomes_clear_the_baseline() {
    let mut conversation = ResponsesWsConversation::new(MAX_STATE);
    let first =
        ResponsesWsIngress::parse(&request_text(None, None, json!([{"index": 1}]))).expect("first");
    let resolved = conversation.resolve(&first, MAX_BODY).expect("resolved");
    conversation.finish_exchange(
        resolved,
        WsResponseOutcome {
            response_id: Some("resp_1".to_owned()),
            completed: false,
            ..WsResponseOutcome::default()
        },
    );
    let second = ResponsesWsIngress::parse(&request_text(Some("resp_1"), None, json!([])))
        .expect("second request");
    assert_eq!(
        conversation.resolve(&second, MAX_BODY).err(),
        Some(WsResolveError::PreviousResponseNotFound)
    );
}

#[test]
fn oversized_state_and_bodies_are_rejected_or_dropped() {
    let mut conversation = ResponsesWsConversation::new(256);
    let first =
        ResponsesWsIngress::parse(&request_text(None, None, json!([{"index": 1}]))).expect("first");
    assert!(matches!(
        conversation.resolve(&first, 16).err(),
        Some(WsResolveError::RequestTooLarge { .. })
    ));

    let resolved = conversation.resolve(&first, MAX_BODY).expect("resolved");
    conversation.finish_exchange(
        resolved,
        WsResponseOutcome {
            response_id: Some("resp_1".to_owned()),
            output_items: Vec::new(),
            output_bytes: 4096,
            completed: true,
        },
    );
    let second = ResponsesWsIngress::parse(&request_text(Some("resp_1"), None, json!([])))
        .expect("second request");
    assert_eq!(
        conversation.resolve(&second, MAX_BODY).err(),
        Some(WsResolveError::PreviousResponseNotFound)
    );
}

#[test]
fn warmup_establishes_a_baseline_for_the_next_incremental_request() {
    let mut conversation = ResponsesWsConversation::new(MAX_STATE);
    let warmup = ResponsesWsIngress::parse(&request_text(
        None,
        Some(false),
        json!([{"role": "user", "index": 1}]),
    ))
    .expect("warmup request");
    assert!(warmup.is_warmup());
    let resolved = conversation.resolve(&warmup, MAX_BODY).expect("resolved");
    conversation.finish_warmup(resolved, "resp_any2api_1".to_owned());

    let turn = ResponsesWsIngress::parse(&request_text(
        Some("resp_any2api_1"),
        None,
        json!([{"role": "user", "index": 2}]),
    ))
    .expect("turn request");
    let body = body_json(&conversation, &turn);
    assert_eq!(
        body["input"],
        json!([{"role": "user", "index": 1}, {"role": "user", "index": 2}])
    );

    conversation.clear();
    assert_eq!(
        conversation.resolve(&turn, MAX_BODY).err(),
        Some(WsResolveError::PreviousResponseNotFound)
    );
}

#[test]
fn egress_frames_forward_json_payloads_and_observe_the_baseline() {
    let item = classify_egress_frame(&SseFrame(Bytes::from_static(
        b"event: response.output_item.done\ndata: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"message\",\"id\":\"item_1\"}}\n\n",
    )));
    assert_eq!(
        item.payload.as_deref(),
        Some(
            br#"{"type":"response.output_item.done","item":{"type":"message","id":"item_1"}}"#
                .as_slice()
        )
    );
    let ResponsesWsObservation::OutputItemDone { item, raw_len } = item.observation else {
        panic!("expected output item observation");
    };
    assert_eq!(item["id"], "item_1");
    assert!(raw_len > 0);

    let completed = classify_egress_frame(&SseFrame(Bytes::from_static(
        b"data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_9\"}}\n\n",
    )));
    let ResponsesWsObservation::Completed { response_id } = completed.observation else {
        panic!("expected completed observation");
    };
    assert_eq!(response_id.as_deref(), Some("resp_9"));

    let delta = classify_egress_frame(&SseFrame(Bytes::from_static(
        b"data: {\"type\":\"response.output_text.delta\",\"delta\":\"hi\"}\n\n",
    )));
    assert!(delta.payload.is_some());
    assert!(matches!(delta.observation, ResponsesWsObservation::None));

    let heartbeat = classify_egress_frame(&SseFrame(Bytes::from_static(b": keep-alive\n\n")));
    assert!(heartbeat.payload.is_none());
    assert!(matches!(
        heartbeat.observation,
        ResponsesWsObservation::None
    ));
}

#[test]
fn wrapped_errors_preserve_status_error_object_and_headers() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-codex-primary-used-percent",
        HeaderValue::from_static("100.0"),
    );
    let event = wrapped_error_event(
        StatusCode::TOO_MANY_REQUESTS,
        &headers,
        br#"{"error":{"type":"usage_limit_reached","message":"limit"}}"#,
    );
    let event: Value = serde_json::from_str(&event).expect("event JSON");
    assert_eq!(event["type"], "error");
    assert_eq!(event["status"], 429);
    assert_eq!(event["error"]["type"], "usage_limit_reached");
    assert_eq!(event["headers"]["x-codex-primary-used-percent"], "100.0");

    let opaque = wrapped_error_event(StatusCode::BAD_GATEWAY, &HeaderMap::new(), b"<html>block");
    let opaque: Value = serde_json::from_str(&opaque).expect("event JSON");
    assert_eq!(opaque["error"]["message"], "<html>block");
    assert!(opaque.get("headers").is_none());

    let recovery: Value =
        serde_json::from_str(&previous_response_not_found_event()).expect("event JSON");
    assert_eq!(
        recovery["error"]["code"], "previous_response_not_found",
        "client retries the full request on this exact code"
    );
}

#[test]
fn warmup_events_carry_the_synthetic_response_id() {
    let created: Value =
        serde_json::from_str(&warmup_created_event("resp_any2api_7")).expect("created JSON");
    assert_eq!(created["type"], "response.created");
    assert_eq!(created["response"]["id"], "resp_any2api_7");
    let completed: Value =
        serde_json::from_str(&warmup_completed_event("resp_any2api_7")).expect("completed JSON");
    assert_eq!(completed["type"], "response.completed");
    assert_eq!(completed["response"]["id"], "resp_any2api_7");
}
