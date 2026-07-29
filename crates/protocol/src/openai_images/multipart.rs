use std::convert::Infallible;

use bytes::{Bytes, BytesMut};
use futures_util::stream;
use http::{HeaderMap, HeaderValue, header};
use multer::Multipart;
use uuid::Uuid;

use crate::{
    ProtocolError,
    api::{MultipartPart, MultipartPayload},
};

/// Parse an in-memory multipart body after the HTTP layer has applied its
/// aggregate request-size limit. Multer still gives us structural boundary,
/// field, and header validation rather than searching binary bytes.
pub(crate) async fn parse(
    body: Bytes,
    content_type: &str,
) -> Result<MultipartPayload, ProtocolError> {
    let boundary = multer::parse_boundary(content_type)
        .map_err(|_| ProtocolError::InvalidPayload("multipart boundary is invalid".into()))?;
    let stream = stream::once(async move { Ok::<Bytes, Infallible>(body) });
    let mut multipart = Multipart::new(stream, boundary);
    let mut parts = Vec::new();
    let mut model_part_index = None;
    let mut stream_value = false;
    let mut stream_seen = false;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| ProtocolError::InvalidPayload("multipart body is malformed".into()))?
    {
        let name = field
            .name()
            .ok_or_else(|| ProtocolError::InvalidPayload("multipart field name is missing".into()))?
            .to_owned();
        let headers = safe_headers(field.headers())?;
        let body = field
            .bytes()
            .await
            .map_err(|_| ProtocolError::InvalidPayload("multipart field is malformed".into()))?;

        if name == "model" {
            if model_part_index.is_some() {
                return Err(ProtocolError::InvalidPayload(
                    "multipart model must appear exactly once".into(),
                ));
            }
            let model = std::str::from_utf8(&body).map_err(|_| {
                ProtocolError::InvalidPayload("multipart model must be valid UTF-8".into())
            })?;
            if model.trim().is_empty() {
                return Err(ProtocolError::InvalidPayload(
                    "multipart model must be non-empty".into(),
                ));
            }
            model_part_index = Some(parts.len());
        } else if name == "stream" {
            if stream_seen {
                return Err(ProtocolError::InvalidPayload(
                    "multipart stream must appear at most once".into(),
                ));
            }
            stream_seen = true;
            let value = std::str::from_utf8(&body).map_err(|_| {
                ProtocolError::InvalidPayload("multipart stream must be valid UTF-8".into())
            })?;
            match value {
                "true" => stream_value = true,
                "false" => stream_value = false,
                _ => {
                    return Err(ProtocolError::InvalidPayload(
                        "multipart stream must be true or false".into(),
                    ));
                }
            }
        }

        parts.push(MultipartPart {
            name,
            headers,
            body,
        });
    }

    let model_part_index = model_part_index.ok_or_else(|| {
        ProtocolError::InvalidPayload("multipart model must appear exactly once".into())
    })?;
    Ok(MultipartPayload {
        parts,
        model_part_index,
        stream: stream_value,
    })
}

/// Encode a parsed body with a fresh boundary and a replacement model value.
/// Part order, duplicate names, bytes, and validated headers are retained.
pub(crate) fn encode(
    mut payload: MultipartPayload,
    upstream_model: &str,
) -> Result<(Bytes, HeaderValue), ProtocolError> {
    let boundary = format!("any2api_{}", Uuid::new_v4().simple());
    let replacement = Bytes::from(upstream_model.to_owned());
    if let Some(part) = payload.parts.get_mut(payload.model_part_index) {
        part.body = replacement;
    } else {
        return Err(ProtocolError::InvalidPayload(
            "multipart model part is missing".into(),
        ));
    }

    let mut output = BytesMut::new();
    for part in payload.parts {
        if part.name.is_empty() {
            return Err(ProtocolError::InvalidPayload(
                "multipart field name is missing".into(),
            ));
        }
        output.extend_from_slice(b"--");
        output.extend_from_slice(boundary.as_bytes());
        output.extend_from_slice(b"\r\n");
        for (name, value) in &part.headers {
            output.extend_from_slice(name.as_str().as_bytes());
            output.extend_from_slice(b": ");
            output.extend_from_slice(value.as_bytes());
            output.extend_from_slice(b"\r\n");
        }
        output.extend_from_slice(b"\r\n");
        output.extend_from_slice(&part.body);
        output.extend_from_slice(b"\r\n");
    }
    output.extend_from_slice(b"--");
    output.extend_from_slice(boundary.as_bytes());
    output.extend_from_slice(b"--\r\n");

    let content_type = format!("multipart/form-data; boundary={boundary}");
    let content_type = HeaderValue::from_str(&content_type)
        .map_err(|_| ProtocolError::InvalidPayload("multipart boundary is invalid".into()))?;
    Ok((output.freeze(), content_type))
}

fn safe_headers(headers: &HeaderMap) -> Result<HeaderMap, ProtocolError> {
    let mut output = HeaderMap::new();
    for (name, value) in headers {
        if forbidden_header(name.as_str()) {
            continue;
        }
        // HeaderMap values have already passed HTTP parser validation. Keep
        // the bytes rather than lossy UTF-8 conversion so binary-safe header
        // handling remains explicit at the re-encoding boundary.
        output.append(name.clone(), value.clone());
    }
    if !output.contains_key(header::CONTENT_DISPOSITION) {
        return Err(ProtocolError::InvalidPayload(
            "multipart field content-disposition is missing".into(),
        ));
    }
    Ok(output)
}

fn forbidden_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "authorization"
            | "api-key"
            | "cookie"
            | "host"
            | "content-length"
            | "connection"
            | "keep-alive"
            | "proxy-connection"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "set-cookie"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
            | "x-api-key"
    )
}
