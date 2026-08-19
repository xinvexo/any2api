use any2api_domain::{ProtocolOperation, RequestSpeedTier};
use bytes::Bytes;
use http::{HeaderMap, HeaderValue};
use serde_json::json;

use super::{
    encode_json_request, encode_raw_json_request, encode_response, extract_affinity,
    request_execution_profile_raw,
};
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
fn extracts_supported_request_speed_tiers_without_changing_wire_encoding() {
    for (operation, field, tier, expected) in [
        (
            ProtocolOperation::Responses,
            "service_tier",
            "priority",
            Some(RequestSpeedTier::Fast),
        ),
        (
            ProtocolOperation::Responses,
            "service_tier",
            "auto",
            Some(RequestSpeedTier::Standard),
        ),
        (
            ProtocolOperation::Messages,
            "speed",
            "fast",
            Some(RequestSpeedTier::Fast),
        ),
        (
            ProtocolOperation::Messages,
            "speed",
            "standard",
            Some(RequestSpeedTier::Standard),
        ),
    ] {
        let mut value = json!({"model":"upstream"});
        value[field] = json!(tier);
        let raw_value = raw(value.clone());
        let raw_encoded =
            encode_raw_json_request(operation, &HeaderMap::new(), &raw_value, "upstream")
                .expect("raw request");
        let structured_encoded =
            encode_json_request(operation, &HeaderMap::new(), &value, "upstream")
                .expect("structured request");

        assert_eq!(raw_encoded.requested_speed_tier, expected);
        assert_eq!(structured_encoded.requested_speed_tier, expected);
    }

    let unknown = json!({"model":"upstream","service_tier":"future"});
    assert_eq!(
        encode_json_request(
            ProtocolOperation::Responses,
            &HeaderMap::new(),
            &unknown,
            "upstream",
        )
        .expect("unknown tier request")
        .requested_speed_tier,
        None
    );
    assert_eq!(
        encode_json_request(
            ProtocolOperation::ImagesGenerations,
            &HeaderMap::new(),
            &json!({"model":"upstream","service_tier":"priority"}),
            "upstream",
        )
        .expect("image request")
        .requested_speed_tier,
        None
    );
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

#[test]
fn large_structured_response_is_encoded_with_the_public_model() {
    let content = "x".repeat(300 * 1024);
    let response = DecodedUpstreamResponse {
        status: http::StatusCode::OK,
        headers: HeaderMap::new(),
        payload: DecodedResponsePayload::StructuredJson(json!({
            "model": "upstream",
            "output": content,
        })),
        telemetry: Default::default(),
    };

    let encoded = encode_response(response, "public").expect("egress response");
    let value: serde_json::Value = serde_json::from_slice(&encoded.body).expect("response JSON");

    assert_eq!(value["model"], "public");
    assert_eq!(value["output"].as_str(), Some(content.as_str()));
}
