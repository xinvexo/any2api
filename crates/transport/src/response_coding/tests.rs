use any2api_domain::RetrySafety;
use async_compression::tokio::write::{BrotliEncoder, GzipEncoder, ZstdEncoder};
use bytes::{Bytes, BytesMut};
use futures_util::{StreamExt, stream};
use http::{HeaderMap, HeaderValue, StatusCode, header};
use tokio::io::AsyncWriteExt;

use super::{decode_response_content, sanitize_request_accept_encoding};
use crate::api::{BoxByteStream, TransportResponse};
use crate::error::{TransportError, TransportErrorStage, TransportFailureScope};

const PAYLOAD: &[u8] = b"event: response.output_text.delta\ndata: {\"delta\":\"hello\"}\n\n";

#[derive(Clone, Copy)]
enum Encoder {
    Gzip,
    Brotli,
    Zstandard,
}

#[test]
fn supported_request_negotiation_is_preserved_and_absence_stays_absent() {
    let mut headers = HeaderMap::new();
    headers.append(
        header::ACCEPT_ENCODING,
        HeaderValue::from_static("GZip; q=0.8, br"),
    );
    headers.append(
        header::ACCEPT_ENCODING,
        HeaderValue::from_static("zstd, identity;q=0"),
    );

    sanitize_request_accept_encoding(&mut headers);

    assert_eq!(
        headers
            .get_all(header::ACCEPT_ENCODING)
            .iter()
            .map(HeaderValue::as_bytes)
            .collect::<Vec<_>>(),
        vec![b"GZip; q=0.8, br".as_slice(), b"zstd, identity;q=0"]
    );

    let mut absent = HeaderMap::new();
    sanitize_request_accept_encoding(&mut absent);
    assert!(!absent.contains_key(header::ACCEPT_ENCODING));
}

#[test]
fn unsupported_or_ambiguous_request_negotiation_is_removed_as_a_whole() {
    for value in ["gzip, deflate", "gzip,", "*", "gzip, *;q=0"] {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::ACCEPT_ENCODING,
            HeaderValue::from_str(value).expect("request coding fixture"),
        );

        sanitize_request_accept_encoding(&mut headers);

        assert!(!headers.contains_key(header::ACCEPT_ENCODING), "{value}");
    }
}

#[tokio::test]
async fn supported_codings_decode_incrementally_and_clear_representation_metadata() {
    for (name, encoder) in [
        ("gzip", Encoder::Gzip),
        ("br", Encoder::Brotli),
        ("zstd", Encoder::Zstandard),
    ] {
        let encoded = encode(encoder, PAYLOAD).await;
        let mut headers = encoded_headers(name, encoded.len());
        headers.insert(
            header::CONTENT_RANGE,
            HeaderValue::from_static("bytes 0-1/2"),
        );
        headers.insert(header::ETAG, HeaderValue::from_static("\"encoded\""));
        headers.insert("content-md5", HeaderValue::from_static("encoded-md5"));
        headers.insert("digest", HeaderValue::from_static("sha-256=encoded"));

        let response =
            decode_response_content(response(StatusCode::OK, headers, byte_chunks(encoded, 1)))
                .expect("supported response coding");

        for removed in [
            header::CONTENT_ENCODING,
            header::CONTENT_LENGTH,
            header::CONTENT_RANGE,
            header::ETAG,
        ] {
            assert!(!response.headers.contains_key(removed));
        }
        assert!(!response.headers.contains_key("content-md5"));
        assert!(!response.headers.contains_key("digest"));
        assert_eq!(collect(response.body).await.expect("decoded body"), PAYLOAD);
    }
}

#[tokio::test]
async fn stacked_codings_are_decoded_in_reverse_declaration_order() {
    let gzip = encode(Encoder::Gzip, PAYLOAD).await;
    let brotli_over_gzip = encode(Encoder::Brotli, &gzip).await;
    let headers = encoded_headers("gzip, br", brotli_over_gzip.len());

    let response = decode_response_content(response(
        StatusCode::OK,
        headers,
        byte_chunks(brotli_over_gzip, 2),
    ))
    .expect("stacked response coding");

    assert_eq!(collect(response.body).await.expect("decoded body"), PAYLOAD);
}

#[tokio::test]
async fn non_success_response_keeps_status_and_decoded_content() {
    let encoded = encode(Encoder::Gzip, PAYLOAD).await;
    let headers = encoded_headers("gzip", encoded.len());

    let response = decode_response_content(response(
        StatusCode::BAD_REQUEST,
        headers,
        byte_chunks(encoded.clone(), 3),
    ))
    .expect("error response content decoding");

    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    assert!(!response.headers.contains_key(header::CONTENT_ENCODING));
    assert_eq!(
        collect(response.body).await.expect("decoded error body"),
        PAYLOAD
    );
}

#[test]
fn unknown_empty_and_overdeep_coding_headers_are_rejected() {
    for (value, message) in [
        ("snappy", "upstream response content coding is unsupported"),
        (
            "gzip,",
            "upstream response content-encoding header was invalid",
        ),
        (
            "gzip, gzip, gzip, gzip, gzip",
            "upstream response content coding chain is too deep",
        ),
    ] {
        let headers = encoded_headers(value, 1);
        let error = match decode_response_content(response(
            StatusCode::OK,
            headers,
            byte_chunks(Bytes::from_static(b"x"), 1),
        )) {
            Ok(_) => panic!("invalid coding must fail"),
            Err(error) => error,
        };
        assert_eq!(error.stage, TransportErrorStage::ReadBody);
        assert_eq!(error.failure_scope, TransportFailureScope::Endpoint);
        assert_eq!(error.retry_safety, RetrySafety::Ambiguous);
        assert_eq!(error.message, message);
    }
}

#[tokio::test]
async fn corrupt_coding_fails_without_exposing_raw_bytes() {
    let headers = encoded_headers("gzip", 7);
    let response = decode_response_content(response(
        StatusCode::OK,
        headers,
        byte_chunks(Bytes::from_static(b"notgzip"), 1),
    ))
    .expect("known coding creates a decoding stream");

    let error = collect(response.body)
        .await
        .expect_err("corrupt gzip must fail");
    assert_eq!(error.stage, TransportErrorStage::ReadBody);
    assert_eq!(error.failure_scope, TransportFailureScope::Endpoint);
    assert_eq!(error.retry_safety, RetrySafety::Ambiguous);
    assert_eq!(
        error.message,
        "upstream response content coding was invalid"
    );
}

#[tokio::test]
async fn underlying_transport_error_keeps_its_original_classification() {
    let original = TransportError::new(
        TransportErrorStage::ReadBody,
        TransportFailureScope::Proxy,
        RetrySafety::RejectedBeforeExecution,
        "sentinel body failure",
    );
    let body: BoxByteStream = Box::pin(stream::iter([Err(original.clone())]));
    let response = decode_response_content(TransportResponse {
        status: StatusCode::OK,
        headers: encoded_headers("gzip", 1),
        body,
        read_failure_scope: TransportFailureScope::Proxy,
    })
    .expect("known coding creates a decoding stream");

    assert_eq!(
        collect(response.body)
            .await
            .expect_err("source error must survive"),
        original
    );
}

async fn encode(encoder: Encoder, payload: &[u8]) -> Bytes {
    match encoder {
        Encoder::Gzip => {
            let mut encoder = GzipEncoder::new(Vec::new());
            encoder.write_all(payload).await.expect("gzip write");
            encoder.shutdown().await.expect("gzip finish");
            Bytes::from(encoder.into_inner())
        }
        Encoder::Brotli => {
            let mut encoder = BrotliEncoder::new(Vec::new());
            encoder.write_all(payload).await.expect("Brotli write");
            encoder.shutdown().await.expect("Brotli finish");
            Bytes::from(encoder.into_inner())
        }
        Encoder::Zstandard => {
            let mut encoder = ZstdEncoder::new(Vec::new());
            encoder.write_all(payload).await.expect("Zstandard write");
            encoder.shutdown().await.expect("Zstandard finish");
            Bytes::from(encoder.into_inner())
        }
    }
}

fn encoded_headers(coding: &str, length: usize) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_ENCODING,
        HeaderValue::from_str(coding).expect("content coding fixture"),
    );
    headers.insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&length.to_string()).expect("content length fixture"),
    );
    headers
}

fn response(status: StatusCode, headers: HeaderMap, body: BoxByteStream) -> TransportResponse {
    TransportResponse {
        status,
        headers,
        body,
        read_failure_scope: TransportFailureScope::Endpoint,
    }
}

fn byte_chunks(bytes: Bytes, chunk_size: usize) -> BoxByteStream {
    let chunks = bytes
        .chunks(chunk_size)
        .map(Bytes::copy_from_slice)
        .map(Ok)
        .collect::<Vec<Result<Bytes, TransportError>>>();
    Box::pin(stream::iter(chunks))
}

async fn collect(mut body: BoxByteStream) -> Result<Bytes, TransportError> {
    let mut collected = BytesMut::new();
    while let Some(chunk) = body.next().await {
        collected.extend_from_slice(&chunk?);
    }
    Ok(collected.freeze())
}
