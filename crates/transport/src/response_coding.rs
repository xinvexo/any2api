//! Generic gateway request negotiation and response content decoding.

use std::{io, pin::Pin};

use any2api_domain::RetrySafety;
use async_compression::tokio::bufread::{BrotliDecoder, GzipDecoder, ZstdDecoder};
use futures_util::{StreamExt, TryStreamExt};
use http::{HeaderMap, header};
use tokio::io::{AsyncRead, BufReader};
use tokio_util::io::{ReaderStream, StreamReader};

use crate::{
    api::{BoxByteStream, TransportResponse},
    error::{TransportError, TransportErrorStage, TransportFailureScope},
    profile::GENERIC_GATEWAY_TRANSPORT_PROFILE as WIRE_PROFILE,
};

type BoxAsyncRead = Pin<Box<dyn AsyncRead + Send + 'static>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ContentCoding {
    Gzip,
    Brotli,
    Zstandard,
}

pub(crate) fn sanitize_request_accept_encoding(headers: &mut HeaderMap) {
    let supported = headers
        .get_all(header::ACCEPT_ENCODING)
        .iter()
        .all(|value| accept_encoding_value_is_supported(value.as_bytes()));
    if !supported {
        headers.remove(header::ACCEPT_ENCODING);
    }
}

fn accept_encoding_value_is_supported(value: &[u8]) -> bool {
    value.split(|byte| *byte == b',').all(|item| {
        let item = trim_ows(item);
        let coding = trim_ows(item.split(|byte| *byte == b';').next().unwrap_or_default());
        !coding.is_empty()
            && [b"gzip".as_slice(), b"br", b"zstd", b"identity"]
                .iter()
                .any(|supported| coding.eq_ignore_ascii_case(supported))
    })
}

pub(crate) fn decode_response_content(
    mut response: TransportResponse,
) -> Result<TransportResponse, TransportError> {
    let codings = parse_content_codings(&response.headers)?;
    if codings.is_empty() {
        response.headers.remove(header::CONTENT_ENCODING);
        return Ok(response);
    }

    remove_encoded_representation_headers(&mut response.headers);
    response.body = decode_body(response.body, &codings);
    Ok(response)
}

fn parse_content_codings(headers: &HeaderMap) -> Result<Vec<ContentCoding>, TransportError> {
    let mut codings = Vec::new();
    for value in headers.get_all(header::CONTENT_ENCODING) {
        for token in value.as_bytes().split(|byte| *byte == b',') {
            let token = trim_ows(token);
            if token.is_empty() {
                return Err(invalid_coding_header());
            }
            if token.eq_ignore_ascii_case(b"identity") {
                continue;
            }
            let coding = if token.eq_ignore_ascii_case(b"gzip") {
                ContentCoding::Gzip
            } else if token.eq_ignore_ascii_case(b"br") {
                ContentCoding::Brotli
            } else if token.eq_ignore_ascii_case(b"zstd") {
                ContentCoding::Zstandard
            } else {
                return Err(unsupported_coding());
            };
            codings.push(coding);
            if codings.len() > WIRE_PROFILE.max_response_content_coding_depth() {
                return Err(coding_chain_too_deep());
            }
        }
    }
    Ok(codings)
}

fn decode_body(body: BoxByteStream, codings: &[ContentCoding]) -> BoxByteStream {
    let stream = body.map_err(io::Error::other);
    let mut reader: BoxAsyncRead = Box::pin(StreamReader::new(stream));
    for coding in codings.iter().rev() {
        let buffered = BufReader::new(reader);
        reader = match coding {
            ContentCoding::Gzip => {
                let mut decoder = GzipDecoder::new(buffered);
                decoder.multiple_members(true);
                Box::pin(decoder)
            }
            ContentCoding::Brotli => Box::pin(BrotliDecoder::new(buffered)),
            ContentCoding::Zstandard => {
                let mut decoder = ZstdDecoder::new(buffered);
                decoder.multiple_members(true);
                Box::pin(decoder)
            }
        };
    }
    Box::pin(ReaderStream::new(reader).map(|result| result.map_err(decode_error)))
}

fn decode_error(error: io::Error) -> TransportError {
    if let Some(error) = error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<TransportError>())
    {
        return error.clone();
    }
    response_coding_error("upstream response content coding was invalid")
}

fn invalid_coding_header() -> TransportError {
    response_coding_error("upstream response content-encoding header was invalid")
}

fn unsupported_coding() -> TransportError {
    response_coding_error("upstream response content coding is unsupported")
}

fn coding_chain_too_deep() -> TransportError {
    response_coding_error("upstream response content coding chain is too deep")
}

fn response_coding_error(message: &'static str) -> TransportError {
    TransportError::new(
        TransportErrorStage::ReadBody,
        TransportFailureScope::Endpoint,
        RetrySafety::Ambiguous,
        message,
    )
}

fn remove_encoded_representation_headers(headers: &mut HeaderMap) {
    for name in [
        header::CONTENT_ENCODING,
        header::CONTENT_LENGTH,
        header::CONTENT_RANGE,
        header::ETAG,
    ] {
        headers.remove(name);
    }
    headers.remove("content-md5");
    headers.remove("digest");
}

fn trim_ows(mut value: &[u8]) -> &[u8] {
    while value
        .first()
        .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
    {
        value = &value[1..];
    }
    while value
        .last()
        .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
    {
        value = &value[..value.len() - 1];
    }
    value
}

#[cfg(test)]
mod tests;
