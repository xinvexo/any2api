use std::convert::Infallible;

use any2api_payload_buffer::{PayloadBuffer, PayloadBufferError};
use bytes::Bytes;
use futures_util::{StreamExt, stream};
use http::{HeaderMap, HeaderValue, header};
use multer::{Field, Multipart};
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
    let max_field_len = body.len();
    let stream = stream::once(async move { Ok::<Bytes, Infallible>(body) });
    let mut multipart = Multipart::new(stream, boundary);
    let mut parts = Vec::new();
    let mut model_part_index = None;
    let mut model_value = None;
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
        let body = read_field_body(field, max_field_len).await?;

        if name == "model" {
            if model_part_index.is_some() {
                return Err(ProtocolError::InvalidPayload(
                    "multipart model must appear exactly once".into(),
                ));
            }
            let model = std::str::from_utf8(&body).map_err(|_| {
                ProtocolError::InvalidPayload("multipart model must be valid UTF-8".into())
            })?;
            let model = model.trim();
            if model.is_empty() {
                return Err(ProtocolError::InvalidPayload(
                    "multipart model must be non-empty".into(),
                ));
            }
            model_part_index = Some(parts.len());
            model_value = Some(model.to_owned());
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
    let model = model_value.ok_or_else(|| {
        ProtocolError::InvalidPayload("multipart model must appear exactly once".into())
    })?;
    Ok(MultipartPayload {
        parts,
        model_part_index,
        model,
        stream: stream_value,
    })
}

/// Encode a parsed body with a fresh boundary and a replacement model value.
/// Part order, duplicate names, bytes, and validated headers are retained.
pub(crate) fn encode(
    payload: &MultipartPayload,
    upstream_model: &str,
) -> Result<(Bytes, HeaderValue), ProtocolError> {
    let boundary = Uuid::new_v4().simple().to_string();
    if payload.parts.get(payload.model_part_index).is_none() {
        return Err(ProtocolError::InvalidPayload(
            "multipart model part is missing".into(),
        ));
    }

    let capacity = encoded_capacity(
        &payload.parts,
        &boundary,
        payload.model_part_index,
        upstream_model.len(),
    )?;
    let mut output =
        PayloadBuffer::with_capacity_hint(Some(capacity), capacity).map_err(encode_buffer_error)?;
    for (index, part) in payload.parts.iter().enumerate() {
        if part.name.is_empty() {
            return Err(ProtocolError::InvalidPayload(
                "multipart field name is missing".into(),
            ));
        }
        write_output(&mut output, b"--")?;
        write_output(&mut output, boundary.as_bytes())?;
        write_output(&mut output, b"\r\n")?;
        for (name, value) in &part.headers {
            write_output(&mut output, name.as_str().as_bytes())?;
            write_output(&mut output, b": ")?;
            write_output(&mut output, value.as_bytes())?;
            write_output(&mut output, b"\r\n")?;
        }
        write_output(&mut output, b"\r\n")?;
        if index == payload.model_part_index {
            write_output(&mut output, upstream_model.as_bytes())?;
        } else {
            write_output(&mut output, &part.body)?;
        }
        write_output(&mut output, b"\r\n")?;
    }
    write_output(&mut output, b"--")?;
    write_output(&mut output, boundary.as_bytes())?;
    write_output(&mut output, b"--\r\n")?;

    let content_type = format!("multipart/form-data; boundary={boundary}");
    let content_type = HeaderValue::from_str(&content_type)
        .map_err(|_| ProtocolError::InvalidPayload("multipart boundary is invalid".into()))?;
    Ok((output.freeze().into_bytes(), content_type))
}

async fn read_field_body(
    mut field: Field<'_>,
    max_field_len: usize,
) -> Result<Bytes, ProtocolError> {
    // Ingress supplies one in-memory chunk, so retain Multer's field slice.
    // The coalescing path only handles a future multi-chunk caller.
    let Some(first) = field
        .next()
        .await
        .transpose()
        .map_err(|_| ProtocolError::InvalidPayload("multipart field is malformed".into()))?
    else {
        return Ok(Bytes::new());
    };
    let Some(second) = field
        .next()
        .await
        .transpose()
        .map_err(|_| ProtocolError::InvalidPayload("multipart field is malformed".into()))?
    else {
        return Ok(first);
    };

    let mut output = PayloadBuffer::with_capacity_hint(
        Some(first.len().saturating_add(second.len())),
        max_field_len,
    )
    .map_err(field_buffer_error)?;
    output
        .extend_from_slice(&first)
        .map_err(field_buffer_error)?;
    output
        .extend_from_slice(&second)
        .map_err(field_buffer_error)?;
    while let Some(chunk) = field
        .next()
        .await
        .transpose()
        .map_err(|_| ProtocolError::InvalidPayload("multipart field is malformed".into()))?
    {
        output
            .extend_from_slice(&chunk)
            .map_err(field_buffer_error)?;
    }
    Ok(output.freeze().into_bytes())
}

fn write_output(output: &mut PayloadBuffer, bytes: &[u8]) -> Result<(), ProtocolError> {
    output.extend_from_slice(bytes).map_err(encode_buffer_error)
}

fn encode_buffer_error(_error: PayloadBufferError) -> ProtocolError {
    ProtocolError::Internal("multipart body buffering failed".into())
}

fn field_buffer_error(error: PayloadBufferError) -> ProtocolError {
    match error {
        PayloadBufferError::TooLarge => {
            ProtocolError::InvalidPayload("multipart field exceeds request body size".into())
        }
        PayloadBufferError::AllocationFailed => {
            ProtocolError::Internal("multipart field buffering failed".into())
        }
    }
}

fn encoded_capacity(
    parts: &[MultipartPart],
    boundary: &str,
    model_part_index: usize,
    replacement_len: usize,
) -> Result<usize, ProtocolError> {
    let mut total = boundary.len().checked_add(6).ok_or_else(size_error)?;
    for (index, part) in parts.iter().enumerate() {
        let body_len = if index == model_part_index {
            replacement_len
        } else {
            part.body.len()
        };
        total = total
            .checked_add(boundary.len().saturating_add(4))
            .and_then(|total| total.checked_add(body_len.saturating_add(4)))
            .ok_or_else(size_error)?;
        for (name, value) in &part.headers {
            total = total
                .checked_add(name.as_str().len())
                .and_then(|total| total.checked_add(value.as_bytes().len().saturating_add(4)))
                .ok_or_else(size_error)?;
        }
    }
    Ok(total)
}

fn size_error() -> ProtocolError {
    ProtocolError::InvalidPayload("multipart body is too large to encode".into())
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
