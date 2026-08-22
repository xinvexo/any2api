use any2api_domain::{
    OpenAiChatCachedTokensField, OpenAiChatCompletionsProfile, OpenAiChatReasoningRequest,
    OpenAiChatRequestField, ProtocolOperation, ProtocolTargetProfile,
};
use serde_json::{Value, json};

use crate::ProtocolError;

use super::{bridged_exchange, chat_frame, decoded, registry, upstream_response};

#[tokio::test]
async fn empty_tools_omit_all_chat_tool_dependent_fields() {
    let registry = registry();
    let request = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({
            "model":"public","input":"hello","tools":[],
            "tool_choice":"auto","parallel_tool_calls":true
        }),
    )
    .await;
    let prepared = bridged_exchange(&registry, ProtocolOperation::Responses)
        .prepare_request(&request, "upstream", None)
        .expect("empty tools request");
    let upstream: Value = serde_json::from_slice(&prepared.request.body).expect("Chat request");
    assert!(upstream.get("tools").is_none());
    assert!(upstream.get("tool_choice").is_none());
    assert!(upstream.get("parallel_tool_calls").is_none());
}

#[tokio::test]
async fn conversation_ledger_rejects_unknown_early_and_mismatched_outputs() {
    let registry = registry();
    for input in [
        json!([{"type":"function_call_output","call_id":"unknown","output":"x"}]),
        json!([
            {"type":"function_call_output","call_id":"call_1","output":"too early"},
            {"type":"function_call","call_id":"call_1","name":"work","arguments":"{}"}
        ]),
        json!([
            {"type":"function_call","call_id":"call_2","name":"work","arguments":"{}"},
            {"type":"custom_tool_call_output","call_id":"call_2","output":"wrong kind"}
        ]),
    ] {
        let request = decoded(
            &registry,
            ProtocolOperation::Responses,
            json!({"model":"public","input":input}),
        )
        .await;
        let error = bridged_exchange(&registry, ProtocolOperation::Responses)
            .prepare_request(&request, "upstream", None)
            .err()
            .expect("invalid ledger must fail before upstream I/O");
        assert!(matches!(error, ProtocolError::InvalidPayload(_)));
    }
}

#[tokio::test]
async fn target_profile_rejects_fields_and_content_it_does_not_declare() {
    let registry = registry();
    let profile = OpenAiChatCompletionsProfile {
        reasoning_request: OpenAiChatReasoningRequest::Unsupported,
        request_fields: OpenAiChatCompletionsProfile::COMPATIBLE_BASELINE
            .request_fields
            .without(OpenAiChatRequestField::Verbosity),
        supports_image_url: false,
        ..OpenAiChatCompletionsProfile::COMPATIBLE_BASELINE
    };
    for body in [
        json!({"model":"public","input":"hello","reasoning":{"effort":"high"}}),
        json!({"model":"public","input":"hello","text":{"verbosity":"low"}}),
        json!({"model":"public","input":[{"type":"message","role":"user","content":[
            {"type":"input_image","image_url":"https://example.com/a.png"}
        ]}]}),
    ] {
        let request = decoded(&registry, ProtocolOperation::Responses, body).await;
        let error = bridged_exchange(&registry, ProtocolOperation::Responses)
            .prepare_request_with_target_profile(
                &request,
                "upstream",
                Some(ProtocolTargetProfile::OpenAiChatCompletions(profile)),
                None,
            )
            .err()
            .expect("undeclared target feature must fail");
        assert!(matches!(error, ProtocolError::InvalidPayload(_)));
    }

    let no_detail = OpenAiChatCompletionsProfile {
        supports_image_url: true,
        supports_image_detail: false,
        ..OpenAiChatCompletionsProfile::COMPATIBLE_BASELINE
    };
    let request = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({"model":"public","input":[{"type":"message","role":"user","content":[
            {"type":"input_image","image_url":"https://example.com/a.png","detail":"high"}
        ]}]}),
    )
    .await;
    let error = bridged_exchange(&registry, ProtocolOperation::Responses)
        .prepare_request_with_target_profile(
            &request,
            "upstream",
            Some(ProtocolTargetProfile::OpenAiChatCompletions(no_detail)),
            None,
        )
        .err()
        .expect("undeclared image detail must fail");
    assert!(matches!(error, ProtocolError::InvalidPayload(_)));
}

#[tokio::test]
async fn target_profile_selects_the_chat_cached_token_layout() {
    let registry = registry();
    let profile = OpenAiChatCompletionsProfile {
        cached_tokens_field: OpenAiChatCachedTokensField::TopLevel,
        ..OpenAiChatCompletionsProfile::COMPATIBLE_BASELINE
    };
    let request = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({"model":"public","input":"hello"}),
    )
    .await;
    let mut exchange = bridged_exchange(&registry, ProtocolOperation::Responses);
    exchange
        .prepare_request_with_target_profile(
            &request,
            "upstream",
            Some(ProtocolTargetProfile::OpenAiChatCompletions(profile)),
            None,
        )
        .expect("request");
    let decoded = exchange
        .decode_upstream_response(upstream_response(json!({
            "choices":[{"index":0,"message":{"role":"assistant","content":"ok"},
                "finish_reason":"stop"}],
            "usage":{"prompt_tokens":8,"completion_tokens":2,"cached_tokens":5}
        })))
        .expect("Kimi-style usage");
    let egress = exchange
        .encode_egress_response(decoded, "public")
        .expect("Responses projection");
    let response: Value = serde_json::from_slice(&egress.body).expect("Responses JSON");
    assert_eq!(
        response["usage"]["input_tokens_details"]["cached_tokens"],
        5
    );
}

#[tokio::test]
async fn buffered_response_rejects_unknown_finish_reason_bad_index_and_malformed_usage() {
    let registry = registry();
    for response in [
        json!({"choices":[{"index":0,"message":{"role":"assistant","content":"x"},
            "finish_reason":"future_reason"}]}),
        json!({"choices":[{"index":1,"message":{"role":"assistant","content":"x"},
            "finish_reason":"stop"}]}),
        json!({"choices":[{"index":0,"message":{"role":"assistant","content":"x"},
            "finish_reason":"stop"}],"usage":{"prompt_tokens":"4","completion_tokens":2}}),
    ] {
        let request = decoded(
            &registry,
            ProtocolOperation::Responses,
            json!({"model":"public","input":"hello"}),
        )
        .await;
        let mut exchange = bridged_exchange(&registry, ProtocolOperation::Responses);
        exchange
            .prepare_request(&request, "upstream", None)
            .expect("request");
        let error = exchange
            .decode_upstream_response(upstream_response(response))
            .expect_err("malformed Chat response must fail");
        assert!(matches!(error, ProtocolError::InvalidPayload(_)));
    }
}

#[tokio::test]
async fn streamed_tool_identity_is_set_once_and_cannot_change() {
    let registry = registry();
    let request = decoded(
        &registry,
        ProtocolOperation::Responses,
        json!({"model":"public","input":"hello","stream":true,"tools":[{
            "type":"function","name":"weather","parameters":{"type":"object"}
        }]}),
    )
    .await;
    let mut exchange = bridged_exchange(&registry, ProtocolOperation::Responses);
    exchange
        .prepare_request(&request, "upstream", None)
        .expect("stream request");
    exchange
        .decode_upstream_event(chat_frame(json!({
            "choices":[{"index":0,"delta":{"tool_calls":[{
                "index":0,"id":"call_1","type":"function",
                "function":{"name":"weather","arguments":"{"}
            }]}}]
        })))
        .expect("first identity fragment");
    let error = exchange
        .decode_upstream_event(chat_frame(json!({
            "choices":[{"index":0,"delta":{"tool_calls":[{
                "index":0,"id":"call_changed","type":"function",
                "function":{"name":"weather","arguments":"}"}
            }]},"finish_reason":"tool_calls"}]
        })))
        .expect_err("changed identity must fail");
    assert!(
        matches!(error, ProtocolError::InvalidPayload(message) if message.contains("id changed"))
    );
}

#[tokio::test]
async fn hosted_and_unloadable_tools_fail_before_upstream_io() {
    let registry = registry();
    for tools in [
        json!([{"type":"web_search"}]),
        json!([{"type":"function","name":"hidden","defer_loading":true,
            "parameters":{"type":"object"}}]),
        json!([{"type":"custom","name":"grammar","format":{
            "type":"grammar","syntax":"lark","definition":"start: x"}}]),
    ] {
        let request = decoded(
            &registry,
            ProtocolOperation::Responses,
            json!({"model":"public","input":"hello","tools":tools}),
        )
        .await;
        let error = bridged_exchange(&registry, ProtocolOperation::Responses)
            .prepare_request(&request, "upstream", None)
            .err()
            .expect("unrepresentable tool must fail");
        assert!(matches!(error, ProtocolError::InvalidPayload(_)));
    }
}

#[tokio::test]
async fn tool_search_history_cannot_authorize_unregistered_or_direct_tools() {
    let registry = registry();
    let search = json!({
        "type":"tool_search","execution":"client","description":"Search tools",
        "parameters":{"type":"object","properties":{"query":{"type":"string"}}}
    });
    let call = json!({
        "type":"tool_search_call","call_id":"search_1","execution":"client",
        "arguments":{"query":"calendar"}
    });
    let cases = [
        json!({
            "model":"public","tools":[],
            "input":[call.clone(),{"type":"tool_search_output","call_id":"search_1",
                "status":"completed","execution":"client","tools":[]}]
        }),
        json!({
            "model":"public","tools":[
                {"type":"function","name":"known","defer_loading":true,
                    "parameters":{"type":"object"}},search.clone()],
            "input":[call.clone(),{"type":"tool_search_output","call_id":"search_1",
                "status":"completed","execution":"client","tools":[{
                    "type":"function","name":"unknown","defer_loading":true,
                    "parameters":{"type":"object"}}]}]
        }),
        json!({
            "model":"public","tools":[
                {"type":"function","name":"known","defer_loading":true,
                    "parameters":{"type":"object"}},search],
            "input":[call,{"type":"tool_search_output","call_id":"search_1",
                "status":"completed","execution":"client","tools":[{
                    "type":"function","name":"known","parameters":{"type":"object"}}]}]
        }),
    ];

    for body in cases {
        let request = decoded(&registry, ProtocolOperation::Responses, body).await;
        let error = bridged_exchange(&registry, ProtocolOperation::Responses)
            .prepare_request(&request, "upstream", None)
            .err()
            .expect("tool search output must not expand its registered authority");
        assert!(matches!(error, ProtocolError::InvalidPayload(_)));
    }
}
