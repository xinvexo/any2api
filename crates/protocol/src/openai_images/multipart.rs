use std::convert::Infallible;

use bytes::{Bytes, BytesMut};
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
        let body = read_field_body(field).await?;

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
    let boundary = format!("any2api_{}", Uuid::new_v4().simple());
    if payload.parts.get(payload.model_part_index).is_none() {
        return Err(ProtocolError::InvalidPayload(
            "multipart model part is missing".into(),
        ));
    }

    let mut output = BytesMut::with_capacity(encoded_capacity(
        &payload.parts,
        &boundary,
        payload.model_part_index,
        upstream_model.len(),
    )?);
    for (index, part) in payload.parts.iter().enumerate() {
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
        if index == payload.model_part_index {
            output.extend_from_slice(upstream_model.as_bytes());
        } else {
            output.extend_from_slice(&part.body);
        }
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

async fn read_field_body(mut field: Field<'_>) -> Result<Bytes, ProtocolError> {
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

    let mut output = BytesMut::with_capacity(first.len().saturating_add(second.len()));
    output.extend_from_slice(&first);
    output.extend_from_slice(&second);
    while let Some(chunk) = field
        .next()
        .await
        .transpose()
        .map_err(|_| ProtocolError::InvalidPayload("multipart field is malformed".into()))?
    {
        output.extend_from_slice(&chunk);
    }
    Ok(output.freeze())
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
