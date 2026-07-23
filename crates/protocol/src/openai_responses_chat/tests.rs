use std::sync::Arc;

use any2api_domain::{ProtocolDialect, ProtocolOperation};
use bytes::Bytes;
use http::{HeaderMap, Method, StatusCode, Uri};
use serde_json::{Value, json};

use super::ResponsesToChatCompletionsBridge;
use crate::{
    OpenAiChatCompletionsAdapter, OpenAiResponsesAdapter, ProtocolError, ProtocolRegistry,
    api::{IngressRequest, SseFrame, UpstreamResponse},
};

#[test]
fn json_bridge_converts_tools_usage_and_previous_response_history() {
    let registry = registry();
    let first = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({
            "model":"public-model",
            "instructions":"Be concise",
            "input":"What is the weather?",
            "reasoning":{"effort":"medium"},
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
    );
    let mut exchange = bridged_exchange(&registry, ProtocolOperation::Responses);
    let prepared = exchange
        .prepare_request(first, "upstream-model")
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
        .hard_affinity_id_from_response(ProtocolOperation::Responses, &decoded_response)
        .expect("response identity")
        .expect("response id");
    let egress = exchange
        .encode_egress_response(decoded_response)
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
    );
    let mut follow_up_exchange = bridged_exchange(&registry, ProtocolOperation::Responses);
    let prepared = follow_up_exchange
        .prepare_request(follow_up, "upstream-model")
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

#[test]
fn streaming_bridge_emits_responses_events_tools_and_usage() {
    let registry = registry();
    let request = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({"model":"public-model","input":"hello","stream":true}),
    );
    let mut exchange = bridged_exchange(&registry, ProtocolOperation::Responses);
    let prepared = exchange
        .prepare_request(request, "upstream-model")
        .expect("stream request");
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
}

#[test]
fn bridge_fails_closed_for_unsupported_or_ambiguous_shapes() {
    let registry = registry();

    for body in [
        json!({"model":"public","input":"hello","unknown":true}),
        json!({"model":"public","input":"hello","n":2}),
        json!({"model":"public","input":"hello","tool_choice":"random"}),
    ] {
        let request = decoded(&registry, ProtocolOperation::Responses, body);
        let error = bridged_exchange(&registry, ProtocolOperation::Responses)
            .prepare_request(request, "upstream")
            .err()
            .expect("unsupported request must fail");
        assert!(matches!(error, ProtocolError::InvalidPayload(_)));
    }

    let lost = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({"model":"public","input":"hello","previous_response_id":"resp_missing"}),
    );
    assert_eq!(
        bridged_exchange(&registry, ProtocolOperation::Responses)
            .prepare_request(lost, "upstream")
            .err()
            .expect("missing history must fail"),
        ProtocolError::SessionBindingLost
    );

    assert!(matches!(
        registry.exchange(
            ProtocolDialect::OpenAiResponses,
            ProtocolDialect::OpenAiChatCompletions,
            ProtocolOperation::ResponsesCompact,
        ),
        Err(ProtocolError::Unsupported(_))
    ));

    let request = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({"model":"public","input":"hello"}),
    );
    let mut exchange = bridged_exchange(&registry, ProtocolOperation::Responses);
    exchange
        .prepare_request(request, "upstream")
        .expect("prepared request");
    let error = exchange
        .decode_upstream_response(upstream_response(json!({
            "choices":[
                {"message":{"role":"assistant","content":"one"}},
                {"message":{"role":"assistant","content":"two"}}
            ]
        })))
        .expect_err("multiple choices must fail");
    assert!(matches!(error, ProtocolError::InvalidPayload(_)));

    let stream_request = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({"model":"public","input":"hello","stream":true}),
    );
    let mut stream_exchange = bridged_exchange(&registry, ProtocolOperation::Responses);
    stream_exchange
        .prepare_request(stream_request, "upstream")
        .expect("prepared stream");
    let error = stream_exchange
        .decode_upstream_event(chat_frame(json!({
            "choices":[{"delta":{"content":"one"}},{"delta":{"content":"two"}}]
        })))
        .expect_err("multiple streamed choices must fail");
    assert!(matches!(error, ProtocolError::InvalidPayload(_)));
}

fn registry() -> ProtocolRegistry {
    let mut registry = ProtocolRegistry::new();
    registry
        .register(Arc::new(OpenAiResponsesAdapter::new()))
        .expect("Responses adapter");
    registry
        .register(Arc::new(OpenAiChatCompletionsAdapter::new()))
        .expect("Chat adapter");
    registry
        .register_bridge(Arc::new(ResponsesToChatCompletionsBridge::new()))
        .expect("Responses to Chat bridge");
    registry
}

fn decoded(
    registry: &ProtocolRegistry,
    operation: ProtocolOperation,
    body: Value,
) -> crate::api::DecodedRequest {
    registry
        .get(operation.dialect())
        .expect("ingress adapter")
        .decode_ingress_request(IngressRequest {
            method: Method::POST,
            uri: Uri::from_static("/v1/test"),
            headers: HeaderMap::new(),
            body: Bytes::from(serde_json::to_vec(&body).expect("request JSON")),
            operation,
        })
        .expect("decoded request")
}

fn bridged_exchange(
    registry: &ProtocolRegistry,
    operation: ProtocolOperation,
) -> crate::api::ProtocolExchange {
    registry
        .exchange(
            ProtocolDialect::OpenAiResponses,
            ProtocolDialect::OpenAiChatCompletions,
            operation,
        )
        .expect("protocol exchange")
}

fn upstream_response(body: Value) -> UpstreamResponse {
    UpstreamResponse {
        status: StatusCode::OK,
        headers: HeaderMap::new(),
        body: Bytes::from(serde_json::to_vec(&body).expect("response JSON")),
    }
}

fn chat_frame(value: Value) -> SseFrame {
    SseFrame(Bytes::from(format!("data: {value}\n\n")))
}
