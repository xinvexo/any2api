use any2api_domain::ProtocolOperation;
use bytes::Bytes;
use http::HeaderMap;
use serde_json::json;

use super::{encode_response, request_execution_profile_raw};
use crate::api::{
    DecodedResponsePayload, DecodedUpstreamResponse, RawJsonPayload, RequestExecutionProfile,
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
