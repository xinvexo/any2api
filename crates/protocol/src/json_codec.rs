use any2api_domain::{
    ProtocolDialect, ProtocolOperation, RequestBodyEncoding, bound_thinking_level,
};
use bytes::Bytes;
use http::{HeaderMap, HeaderValue, Method, Uri, header};
use serde_json::Value;

use crate::{
    ProtocolError, affinity,
    api::{
        AdapterPayload, DecodedRequest, DecodedUpstreamResponse, EgressResponse,
        EncodedUpstreamRequest, IngressAffinity, IngressRequest, RawJsonPayload,
        RequestExecutionProfile,
    },
    raw_json::{object_field, raw_array, raw_string},
};

mod request_encoding;

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
        .ok_or_else(|| ProtocolError::InvalidPayload("model must be a non-empty string".into()))?
        .to_owned();
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
        affinity,
        payload: AdapterPayload::RawJson(payload),
    })
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

/// Restore the public model name and emit the egress body, reusing the
/// original wire bytes whenever nothing had to change.
pub(crate) fn encode_response(
    response: DecodedUpstreamResponse,
    public_model: &str,
) -> Result<EgressResponse, ProtocolError> {
    let DecodedUpstreamResponse {
        status,
        headers,
        body,
        mut parsed,
        ..
    } = response;
    let public = Value::String(public_model.to_owned());
    let rewritten = match parsed
        .as_object_mut()
        .and_then(|object| object.get_mut("model"))
    {
        Some(model) if *model != public => {
            *model = public;
            true
        }
        _ => false,
    };
    let body = match body.filter(|_| !rewritten) {
        Some(body) => body,
        None => serde_json::to_vec(&parsed).map(Bytes::from).map_err(|_| {
            ProtocolError::InvalidPayload("egress response could not be encoded".into())
        })?,
    };
    Ok(EgressResponse {
        status,
        headers,
        body,
    })
}

#[cfg(test)]
mod tests {
    use any2api_domain::ProtocolOperation;
    use bytes::Bytes;
    use serde_json::json;

    use super::request_execution_profile_raw;
    use crate::api::{RawJsonPayload, RequestExecutionProfile};

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
}
