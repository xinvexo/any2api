use any2api_domain::{ProtocolDialect, ProtocolOperation};
use bytes::Bytes;
use http::{HeaderMap, Method, Uri};
use serde_json::json;

use super::{decode_request, encode_response, request_execution_profile_raw};
use crate::api::{
    DecodedResponsePayload, DecodedUpstreamResponse, IngressRequest, RawJsonPayload,
    RequestExecutionProfile,
};

fn raw(value: serde_json::Value) -> RawJsonPayload {
    RawJsonPayload::parse(Bytes::from(
        serde_json::to_vec(&value).expect("encode request"),
    ))
    .expect("raw request")
}

#[test]
fn only_a_final_responses_compaction_trigger_selects_the_remote_profile() {
    let remote = json!({
        "input": [
            {"type":"message","role":"user","content":"hello"},
            {"type":"compaction_trigger"}
        ]
    });
    assert_eq!(
        request_execution_profile_raw(ProtocolOperation::Responses, &raw(remote)),
        RequestExecutionProfile::RemoteCompaction
    );

    for ordinary in [
        json!({"input":[{"type":"compaction_trigger"},{"type":"message"}]}),
        json!({"input":[{"type":"message","content":{"type":"compaction_trigger"}}]}),
        json!({"input":"compaction_trigger"}),
    ] {
        assert_eq!(
            request_execution_profile_raw(ProtocolOperation::Responses, &raw(ordinary)),
            RequestExecutionProfile::Standard
        );
    }
}

#[test]
fn responses_compact_always_uses_the_remote_profile() {
    let request = json!({"input":[]});
    assert_eq!(
        request_execution_profile_raw(ProtocolOperation::ResponsesCompact, &raw(request)),
        RequestExecutionProfile::RemoteCompaction
    );
}

#[test]
fn openai_text_requests_extract_only_non_empty_prompt_cache_keys() {
    let decoded = decode(json!({
        "model":"gpt-test",
        "input":"hello",
        "prompt_cache_key":"private-cache-key"
    }));
    assert_eq!(
        decoded.prompt_cache_key.as_deref(),
        Some("private-cache-key")
    );
    assert!(!format!("{decoded:?}").contains("private-cache-key"));

    for value in [serde_json::Value::Null, json!("")] {
        let decoded = decode(json!({
            "model":"gpt-test",
            "input":"hello",
            "prompt_cache_key":value
        }));
        assert!(decoded.prompt_cache_key.is_none());
    }
}

#[test]
fn invalid_prompt_cache_keys_are_rejected() {
    let invalid_type = decode_request(
        ingress(json!({
            "model":"gpt-test",
            "input":"hello",
            "prompt_cache_key":42
        })),
        ProtocolDialect::OpenAiResponses,
    );
    assert!(invalid_type.is_err());

    let too_long = "x".repeat(super::MAX_PROMPT_CACHE_KEY_BYTES + 1);
    let oversized = decode_request(
        ingress(json!({
            "model":"gpt-test",
            "input":"hello",
            "prompt_cache_key":too_long
        })),
        ProtocolDialect::OpenAiResponses,
    );
    assert!(oversized.is_err());
}

fn decode(body: serde_json::Value) -> crate::api::DecodedRequest {
    decode_request(ingress(body), ProtocolDialect::OpenAiResponses)
        .expect("valid Responses request")
}

fn ingress(body: serde_json::Value) -> IngressRequest {
    IngressRequest {
        method: Method::POST,
        uri: Uri::from_static("/v1/responses"),
        headers: HeaderMap::new(),
        body: Bytes::from(serde_json::to_vec(&body).expect("request JSON")),
        operation: ProtocolOperation::Responses,
    }
}

fn decoded_response(body: Bytes) -> DecodedUpstreamResponse {
    DecodedUpstreamResponse {
        status: http::StatusCode::OK,
        headers: HeaderMap::new(),
        payload: DecodedResponsePayload::RawJson(body),
        telemetry: Default::default(),
    }
}

#[test]
fn model_restore_splices_bytes_keeping_key_order_and_big_integers() {
    let body = Bytes::from_static(
        br#"{"z":9007199254740993,"model":"upstream","a":{"model":"nested"},"big":1.2300}"#,
    );
    let encoded = encode_response(decoded_response(body), "public").expect("egress response");
    assert_eq!(
        encoded.body,
        Bytes::from_static(
            br#"{"z":9007199254740993,"model":"public","a":{"model":"nested"},"big":1.2300}"#,
        )
    );
}

#[test]
fn matching_model_reuses_the_upstream_wire_bytes() {
    let body = Bytes::from_static(br#"{"model":"public","big":9007199254740993}"#);
    let encoded =
        encode_response(decoded_response(body.clone()), "public").expect("egress response");
    assert_eq!(encoded.body.as_ptr(), body.as_ptr());
}
