use any2api_domain::ProtocolOperation;
use bytes::Bytes;
use http::{HeaderMap, HeaderValue};
use serde_json::json;

use super::{encode_response, extract_affinity, request_execution_profile_raw};
use crate::api::{
    DecodedResponsePayload, DecodedUpstreamResponse, IngressAffinity, RawJsonPayload,
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
fn alpha_search_body_id_shares_the_codex_session_namespace() {
    let headers = HeaderMap::new();
    let search = raw(json!({"id":"0199-codex-session","model":"gpt","input":[]}));
    assert_eq!(
        extract_affinity(ProtocolOperation::AlphaSearch, &headers, &search).expect("affinity"),
        IngressAffinity::Session("codex:0199-codex-session".into())
    );

    let mut session_headers = HeaderMap::new();
    session_headers.insert("session_id", HeaderValue::from_static("0199-codex-session"));
    let responses = raw(json!({"model":"gpt","input":[]}));
    assert_eq!(
        extract_affinity(ProtocolOperation::Responses, &session_headers, &responses)
            .expect("affinity"),
        IngressAffinity::Session("codex:0199-codex-session".into())
    );
}

#[test]
fn alpha_search_without_an_id_falls_back_to_explicit_session_headers() {
    let missing = raw(json!({"model":"gpt"}));
    assert_eq!(
        extract_affinity(ProtocolOperation::AlphaSearch, &HeaderMap::new(), &missing)
            .expect("affinity"),
        IngressAffinity::None
    );

    let mut explicit = HeaderMap::new();
    explicit.insert("x-any2api-session", HeaderValue::from_static("manual"));
    assert_eq!(
        extract_affinity(ProtocolOperation::AlphaSearch, &explicit, &missing).expect("affinity"),
        IngressAffinity::Session("any2api:manual".into())
    );

    let invalid = raw(json!({"id":42,"model":"gpt"}));
    assert!(extract_affinity(ProtocolOperation::AlphaSearch, &HeaderMap::new(), &invalid).is_err());
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
