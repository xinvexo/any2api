use any2api_domain::{
    ProtocolDialect, ProtocolOperation, RequestBodyEncoding, bound_thinking_level,
};
use bytes::Bytes;
use http::{HeaderMap, HeaderValue, Method, Uri, header};
use serde_json::Value;

use crate::{
    ProtocolError, affinity,
    api::{
        AdapterPayload, DecodedRequest, DecodedResponsePayload, DecodedUpstreamResponse,
        EgressResponse, EncodedUpstreamRequest, IngressAffinity, IngressRequest,
        ProtocolResponseTelemetry, RawJsonPayload, RequestExecutionProfile, UpstreamResponse,
    },
    raw_json::{
        json_string, object_field, object_field_raw, raw_array, raw_string, splice_ranges,
        subslice_range,
    },
};

mod request_encoding;

#[cfg(test)]
mod tests;

const MAX_PROMPT_CACHE_KEY_BYTES: usize = 4 * 1024;

pub(crate) fn decode_request(
    request: IngressRequest,
    dialect: ProtocolDialect,
) -> Result<DecodedRequest, ProtocolError> {
    let IngressRequest {
        method,
        headers,
        body,
        operation,
        ..
    } = request;
    if method != Method::POST || operation.dialect() != dialect {
        return Err(ProtocolError::Unsupported(format!("{:?}", operation)));
    }
    let payload = RawJsonPayload::parse(body)?;
    let model = payload
        .parse_field::<String>("model")
        .transpose()
        .map_err(|_| ProtocolError::InvalidPayload("model must be a non-empty string".into()))?
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ProtocolError::InvalidPayload("model must be a non-empty string".into()))?;
    let stream = match payload.parse_field::<bool>("stream") {
        Some(Ok(value)) => value,
        Some(Err(_)) => {
            return Err(ProtocolError::InvalidPayload(
                "stream must be a boolean".into(),
            ));
        }
        None => false,
    };
    if stream && !operation.allows_stream() {
        return Err(ProtocolError::InvalidPayload(
            "this operation does not support streaming".into(),
        ));
    }
    let affinity = extract_affinity(operation, &headers, &payload)?;
    let thinking_level = extract_raw_thinking_level(dialect, &payload);
    let prompt_cache_key = extract_prompt_cache_key(operation, &payload)?;
    let body_encoding = request_body_encoding(&headers)?;
    let execution_profile = request_execution_profile_raw(operation, &payload);

    Ok(DecodedRequest {
        dialect,
        operation,
        execution_profile,
        client_headers: headers,
        headers: HeaderMap::new(),
        body_encoding,
        model: Some(model),
        stream,
        thinking_level,
        prompt_cache_key,
        affinity,
        payload: AdapterPayload::RawJson(payload),
    })
}

fn extract_prompt_cache_key(
    operation: ProtocolOperation,
    payload: &RawJsonPayload,
) -> Result<Option<String>, ProtocolError> {
    if !matches!(
        operation,
        ProtocolOperation::Responses | ProtocolOperation::ChatCompletions
    ) {
        return Ok(None);
    }
    let value = payload
        .parse_field::<Option<String>>("prompt_cache_key")
        .transpose()
        .map_err(|_| {
            ProtocolError::InvalidPayload("prompt_cache_key must be a string or null".into())
        })?
        .flatten()
        .filter(|value| !value.is_empty());
    if value
        .as_ref()
        .is_some_and(|value| value.len() > MAX_PROMPT_CACHE_KEY_BYTES)
    {
        return Err(ProtocolError::InvalidPayload(
            "prompt_cache_key is too long".into(),
        ));
    }
    Ok(value)
}

fn extract_affinity(
    operation: ProtocolOperation,
    headers: &HeaderMap,
    payload: &RawJsonPayload,
) -> Result<IngressAffinity, ProtocolError> {
    if matches!(
        operation,
        ProtocolOperation::ImagesGenerations
            | ProtocolOperation::ImagesEdits
            | ProtocolOperation::MessagesCountTokens
    ) {
        return Ok(IngressAffinity::None);
    }
    let previous = if operation == ProtocolOperation::Responses {
        payload
            .parse_field::<Option<String>>("previous_response_id")
            .transpose()
            .map_err(|_| {
                ProtocolError::InvalidPayload(
                    "previous_response_id must be a string or null".into(),
                )
            })?
            .flatten()
    } else {
        None
    };
    let claude_user_id = payload
        .field("metadata")
        .and_then(|metadata| object_field(metadata, "user_id"))
        .and_then(raw_string);
    let conversation_id = payload.field("conversation_id").and_then(raw_string);
    affinity::extract_parts(
        operation,
        headers,
        previous.as_deref(),
        claude_user_id.as_deref(),
        conversation_id.as_deref(),
    )
}

fn request_execution_profile_raw(
    operation: ProtocolOperation,
    payload: &RawJsonPayload,
) -> RequestExecutionProfile {
    if operation == ProtocolOperation::ResponsesCompact {
        return RequestExecutionProfile::RemoteCompaction;
    }
    let remote = operation == ProtocolOperation::Responses
        && payload
            .field("input")
            .and_then(raw_array)
            .and_then(|items| items.last().copied())
            .and_then(|item| object_field(item.get().as_bytes(), "type"))
            .and_then(raw_string)
            .as_deref()
            == Some("compaction_trigger");
    if remote {
        RequestExecutionProfile::RemoteCompaction
    } else {
        RequestExecutionProfile::Standard
    }
}

fn extract_raw_thinking_level(
    dialect: ProtocolDialect,
    payload: &RawJsonPayload,
) -> Option<String> {
    let effort = match dialect {
        ProtocolDialect::OpenAiResponses => payload
            .field("reasoning")
            .and_then(|value| object_field(value, "effort"))
            .and_then(raw_string),
        ProtocolDialect::OpenAiChatCompletions => {
            payload.field("reasoning_effort").and_then(raw_string)
        }
        ProtocolDialect::AnthropicMessages => payload
            .field("output_config")
            .and_then(|value| object_field(value, "effort"))
            .and_then(raw_string),
        ProtocolDialect::OpenAiImages => None,
    };
    effort.and_then(bound_thinking_level)
}

fn request_body_encoding(headers: &HeaderMap) -> Result<RequestBodyEncoding, ProtocolError> {
    let mut values = headers.get_all(header::CONTENT_ENCODING).iter();
    let Some(value) = values.next() else {
        return Ok(RequestBodyEncoding::Identity);
    };
    if values.next().is_some() {
        return Err(ProtocolError::InvalidPayload(
            "content-encoding must appear at most once".into(),
        ));
    }
    let value = value
        .to_str()
        .map_err(|_| ProtocolError::InvalidPayload("content-encoding is invalid".into()))?;
    match value.trim().to_ascii_lowercase().as_str() {
        "identity" => Ok(RequestBodyEncoding::Identity),
        "zstd" => Ok(RequestBodyEncoding::Zstd),
        _ => Err(ProtocolError::InvalidPayload(
            "content-encoding is not supported".into(),
        )),
    }
}

pub(crate) fn encode_request(
    operation: ProtocolOperation,
    forwarded: &HeaderMap,
    payload: &AdapterPayload,
    upstream_model: &str,
) -> Result<EncodedUpstreamRequest, ProtocolError> {
    match payload {
        AdapterPayload::Json(value) => {
            encode_json_request(operation, forwarded, value, upstream_model)
        }
        AdapterPayload::RawJson(value) => {
            encode_raw_json_request(operation, forwarded, value, upstream_model)
        }
        AdapterPayload::Multipart(_) => Err(ProtocolError::InvalidPayload(
            "request body must be JSON".into(),
        )),
    }
}

fn encode_raw_json_request(
    operation: ProtocolOperation,
    forwarded: &HeaderMap,
    value: &RawJsonPayload,
    upstream_model: &str,
) -> Result<EncodedUpstreamRequest, ProtocolError> {
    let stream = value
        .parse_field::<bool>("stream")
        .transpose()
        .map_err(|_| ProtocolError::InvalidPayload("stream must be a boolean".into()))?
        .unwrap_or(false);
    let body = value.encode(operation, upstream_model)?;
    let mut headers = forwarded.clone();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(
        header::ACCEPT,
        HeaderValue::from_static(if stream {
            "text/event-stream"
        } else {
            "application/json"
        }),
    );
    Ok(EncodedUpstreamRequest {
        method: Method::POST,
        uri: Uri::from_static("/"),
        headers,
        body,
    })
}

pub(crate) fn encode_json_request(
    operation: ProtocolOperation,
    forwarded: &HeaderMap,
    value: &Value,
    upstream_model: &str,
) -> Result<EncodedUpstreamRequest, ProtocolError> {
    let object = value.as_object().ok_or_else(|| {
        ProtocolError::InvalidPayload("request body must be a JSON object".into())
    })?;
    let stream = object
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let body = request_encoding::encode(operation, value, upstream_model)?;
    let mut headers = forwarded.clone();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(
        header::ACCEPT,
        HeaderValue::from_static(if stream {
            "text/event-stream"
        } else {
            "application/json"
        }),
    );

    Ok(EncodedUpstreamRequest {
        method: Method::POST,
        uri: Uri::from_static("/"),
        headers,
        body,
    })
}

pub(crate) fn parse_response_body(body: &Bytes) -> Result<Value, ProtocolError> {
    serde_json::from_slice(body)
        .map_err(|_| ProtocolError::InvalidPayload("upstream response must be valid JSON".into()))
}

pub(crate) fn decode_direct_response(
    response: UpstreamResponse,
    telemetry: impl FnOnce(&[u8]) -> ProtocolResponseTelemetry,
) -> Result<DecodedUpstreamResponse, ProtocolError> {
    serde_json::from_slice::<serde::de::IgnoredAny>(&response.body).map_err(|_| {
        ProtocolError::InvalidPayload("upstream response must be valid JSON".into())
    })?;
    let telemetry = telemetry(&response.body);
    Ok(DecodedUpstreamResponse {
        status: response.status,
        headers: response.headers,
        payload: DecodedResponsePayload::RawJson(response.body),
        telemetry,
    })
}

/// Restore the public model name and emit the egress body. The original wire
/// bytes are reused untouched when the model already matches, and only the
/// model value's byte range is spliced otherwise — key order and number
/// formatting pass through verbatim.
pub(crate) fn encode_response(
    response: DecodedUpstreamResponse,
    public_model: &str,
) -> Result<EgressResponse, ProtocolError> {
    let DecodedUpstreamResponse {
        status,
        headers,
        payload,
        ..
    } = response;
    let body = match payload {
        DecodedResponsePayload::RawJson(body) => rewrite_body_model(body, public_model)?,
        DecodedResponsePayload::StructuredJson(mut parsed) => {
            let public = Value::String(public_model.to_owned());
            if let Some(model) = parsed
                .as_object_mut()
                .and_then(|object| object.get_mut("model"))
                && *model != public
            {
                *model = public;
            }
            serde_json::to_vec(&parsed).map(Bytes::from).map_err(|_| {
                ProtocolError::InvalidPayload("egress response could not be encoded".into())
            })?
        }
    };
    Ok(EgressResponse {
        status,
        headers,
        body,
    })
}

fn rewrite_body_model(body: Bytes, public_model: &str) -> Result<Bytes, ProtocolError> {
    let Some(model) = object_field_raw(&body, "model") else {
        return Ok(body);
    };
    if json_string(model).as_deref() == Some(public_model) {
        return Ok(body);
    }
    let Some(range) = subslice_range(&body, model.get().as_bytes()) else {
        return Ok(body);
    };
    let replacement = serde_json::to_string(public_model).map_err(|_| {
        ProtocolError::InvalidPayload("egress response could not be encoded".into())
    })?;
    Ok(splice_ranges(&body, &[range], replacement.as_bytes()))
}
