use any2api_domain::{ProtocolDialect, PublicErrorCode};
use any2api_protocol::api::ProtocolError;
use bytes::Bytes;
use http::{HeaderMap, HeaderValue, header};

use super::{encode_zstd, pass_through_accept_encoding, protocol_request_error};

#[test]
fn accept_encoding_is_copied_only_for_same_dialect_requests() {
    let mut client = HeaderMap::new();
    client.append(
        header::ACCEPT_ENCODING,
        HeaderValue::from_static("gzip; q=0.8, br"),
    );
    client.append(header::ACCEPT_ENCODING, HeaderValue::from_static("zstd"));
    let mut direct = HeaderMap::new();
    direct.insert(
        header::ACCEPT_ENCODING,
        HeaderValue::from_static("gateway-default"),
    );

    pass_through_accept_encoding(
        &client,
        &mut direct,
        ProtocolDialect::OpenAiResponses,
        ProtocolDialect::OpenAiResponses,
    );

    assert_eq!(
        direct
            .get_all(header::ACCEPT_ENCODING)
            .iter()
            .map(HeaderValue::as_bytes)
            .collect::<Vec<_>>(),
        vec![b"gzip; q=0.8, br".as_slice(), b"zstd"]
    );

    pass_through_accept_encoding(
        &client,
        &mut direct,
        ProtocolDialect::OpenAiResponses,
        ProtocolDialect::OpenAiChatCompletions,
    );
    assert!(!direct.contains_key(header::ACCEPT_ENCODING));

    pass_through_accept_encoding(
        &HeaderMap::new(),
        &mut direct,
        ProtocolDialect::OpenAiResponses,
        ProtocolDialect::OpenAiResponses,
    );
    assert!(!direct.contains_key(header::ACCEPT_ENCODING));
}

#[test]
fn bridged_invalid_payload_keeps_the_protocol_pair_and_diagnostic() {
    let error = protocol_request_error(
        ProtocolDialect::OpenAiResponses,
        ProtocolDialect::OpenAiChatCompletions,
        ProtocolError::InvalidPayload("unsupported field `text.future`".into()),
    );

    assert_eq!(error.code(), PublicErrorCode::InvalidRequest);
    assert_eq!(
        error.client_message(),
        "cannot bridge OpenAI Responses to Chat Completions: unsupported field `text.future`"
    );

    let direct = protocol_request_error(
        ProtocolDialect::OpenAiResponses,
        ProtocolDialect::OpenAiResponses,
        ProtocolError::InvalidPayload("request detail".into()),
    );
    assert_eq!(
        direct.client_message(),
        "request cannot be represented by the configured upstream protocol"
    );
}

#[test]
fn zstd_encoding_streams_large_output_into_the_payload_buffer() {
    let mut state = 0x9e37_79b9_u32;
    let input = (0..512 * 1024)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect::<Vec<_>>();

    let encoded = encode_zstd(Bytes::copy_from_slice(&input)).expect("zstd encode");
    assert!(encoded.len() >= 256 * 1024);
    let decoded = zstd::stream::decode_all(encoded.as_ref()).expect("zstd decode");
    assert_eq!(decoded, input);
}
