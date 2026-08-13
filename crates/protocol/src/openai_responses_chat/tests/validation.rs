use any2api_domain::{ProtocolDialect, ProtocolOperation};
use serde_json::{Value, json};

use crate::{ProtocolError, api::MAX_BRIDGE_CONTINUATION_STATE_BYTES};

use super::{bridged_exchange, chat_frame, decoded, registry, upstream_response};

#[tokio::test]
async fn bridge_fails_closed_for_unsupported_or_ambiguous_shapes() {
    let registry = registry();

    for body in [
        json!({"model":"public","input":"hello","unknown":true}),
        json!({"model":"public","input":"hello","n":2}),
        json!({"model":"public","input":"hello","tool_choice":"random"}),
        json!({"model":"public","input":"hello","include":["message.output_text.logprobs"]}),
        json!({"model":"public","input":"hello","reasoning":{"summary":"verbose"}}),
        json!({"model":"public","input":"hello","reasoning":{"effort":"high","future":true}}),
        json!({"model":"public","input":"hello","text":{"verbosity":"maximum"}}),
        json!({"model":"public","input":"hello","client_metadata":"opaque"}),
        json!({"model":"public","input":"hello","client_metadata":{"turn":42}}),
        json!({"model":"public","input":"hello","prompt_cache_key":null}),
        json!({"model":"public","input":[{"type":"message","role":"user","content":[{"type":"input_image","image_url":"https://example.com/image.png","detail":"maximum"}]}]}),
        json!({"model":"public","input":[
            {"type":"function_call","call_id":"call_1","name":"tool","arguments":"{}"},
            {"type":"function_call","call_id":"call_1","name":"tool","arguments":"{}"}
        ]}),
    ] {
        let request = decoded(&registry, ProtocolOperation::Responses, body).await;
        let error = bridged_exchange(&registry, ProtocolOperation::Responses)
            .prepare_request(&request, "upstream", None)
            .err()
            .expect("unsupported request must fail");
        assert!(matches!(error, ProtocolError::InvalidPayload(_)));
    }

    let lost = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({"model":"public","input":"hello","previous_response_id":"resp_missing"}),
    )
    .await;
    assert_eq!(
        bridged_exchange(&registry, ProtocolOperation::Responses)
            .prepare_request(&lost, "upstream", None)
            .err()
            .expect("missing history must fail"),
        ProtocolError::SessionBindingLost
    );

    let oversized = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({
            "model":"public",
            "input":"x".repeat(MAX_BRIDGE_CONTINUATION_STATE_BYTES)
        }),
    )
    .await;
    assert!(matches!(
        bridged_exchange(&registry, ProtocolOperation::Responses)
            .prepare_request(&oversized, "upstream", None),
        Err(ProtocolError::ContinuationTooLarge { .. })
    ));

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
    )
    .await;
    let mut exchange = bridged_exchange(&registry, ProtocolOperation::Responses);
    exchange
        .prepare_request(&request, "upstream", None)
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
    )
    .await;
    let mut stream_exchange = bridged_exchange(&registry, ProtocolOperation::Responses);
    stream_exchange
        .prepare_request(&stream_request, "upstream", None)
        .expect("prepared stream");
    let error = stream_exchange
        .decode_upstream_event(chat_frame(json!({
            "choices":[{"delta":{"content":"one"}},{"delta":{"content":"two"}}]
        })))
        .expect_err("multiple streamed choices must fail");
    assert!(matches!(error, ProtocolError::InvalidPayload(_)));
}

#[tokio::test]
async fn bridge_diagnostics_identify_the_unsupported_field_or_type() {
    for (body, expected) in [
        (
            json!({"model":"public","input":"hello","future":true}),
            "unsupported field `future`",
        ),
        (
            json!({"model":"public","input":"hello","text":{"future":true}}),
            "unsupported field `text.future`",
        ),
        (
            json!({"model":"public","input":"hello","tools":[{
                "type":"computer",
                "display_width":1024,
                "display_height":768
            }]}),
            "tool type `computer` is unsupported",
        ),
        (
            json!({"model":"public","input":[{"type":"computer_call"}]}),
            "input item type `computer_call` is unsupported",
        ),
    ] {
        let registry = registry();
        let request = decoded(&registry, ProtocolOperation::Responses, body).await;
        let error = bridged_exchange(&registry, ProtocolOperation::Responses)
            .prepare_request(&request, "upstream", None)
            .err()
            .expect("unsupported request must fail before upstream I/O");

        assert_eq!(error, ProtocolError::InvalidPayload(expected.into()));
    }
}

#[tokio::test]
async fn bridge_accepts_audited_response_projection_hints_for_any_provider() {
    let registry = registry();
    let request = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({
            "model":"public",
            "input":"title this session",
            "prompt_cache_key":"shared-session-prefix",
            "reasoning":{"summary":"concise"},
            "include":["reasoning.encrypted_content"],
            "client_metadata":{"x-codex-turn-metadata":"opaque"}
        }),
    )
    .await;
    let prepared = bridged_exchange(&registry, ProtocolOperation::Responses)
        .prepare_request(&request, "upstream", None)
        .expect("projection hints are supported by the protocol bridge");
    let upstream: Value =
        serde_json::from_slice(&prepared.request.body).expect("upstream request JSON");

    assert_eq!(upstream["messages"][0]["content"], "title this session");
    assert_eq!(upstream["prompt_cache_key"], "shared-session-prefix");
    assert!(upstream.get("reasoning_effort").is_none());
    assert!(upstream.get("reasoning").is_none());
    assert!(upstream.get("include").is_none());
    assert!(upstream.get("client_metadata").is_none());
}

#[tokio::test]
async fn bridge_rejects_function_calls_without_matching_outputs() {
    for input in [
        json!([
            {"type":"function_call","call_id":"call_interrupted","name":"work","arguments":"{}"},
            {"type":"message","role":"user","content":"continue without the tool"}
        ]),
        json!([
            {"type":"function_call","call_id":"call_a","name":"alpha","arguments":"{}"},
            {"type":"function_call","call_id":"call_b","name":"beta","arguments":"{}"},
            {"type":"function_call_output","call_id":"call_b","output":"B"},
            {"type":"message","role":"user","content":"continue"}
        ]),
    ] {
        let registry = registry();
        let request = decoded(
            &registry,
            ProtocolOperation::Responses,
            json!({"model":"public","input":input}),
        )
        .await;
        let error = bridged_exchange(&registry, ProtocolOperation::Responses)
            .prepare_request(&request, "upstream", None)
            .err()
            .expect("orphan function call must fail before encoding");
        assert!(
            matches!(error, ProtocolError::InvalidPayload(message) if message.contains("matching function_call_output"))
        );
    }
}
