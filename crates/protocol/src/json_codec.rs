use any2api_domain::{
    ProtocolDialect, ProtocolOperation, RequestBodyEncoding, bound_thinking_level,
};
use bytes::Bytes;
use http::{HeaderMap, HeaderValue, Method, Uri, header};
use serde_json::{Map, Value};

use crate::{
    ProtocolError, affinity,
    api::{
        AdapterPayload, DecodedRequest, DecodedUpstreamResponse, EgressResponse,
        EncodedUpstreamRequest, IngressRequest,
    },
};

pub(crate) fn decode_request(
    request: IngressRequest,
    dialect: ProtocolDialect,
) -> Result<DecodedRequest, ProtocolError> {
    if request.method != Method::POST || request.operation.dialect() != dialect {
        return Err(ProtocolError::Unsupported(format!(
            "{:?}",
            request.operation
        )));
    }

    let value: Value = serde_json::from_slice(&request.body)
        .map_err(|_| ProtocolError::InvalidPayload("request body must be valid JSON".into()))?;
    let object = value.as_object().ok_or_else(|| {
        ProtocolError::InvalidPayload("request body must be a JSON object".into())
    })?;
    let model = object
        .get("model")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ProtocolError::InvalidPayload("model must be a non-empty string".into()))?
        .to_owned();
    let stream = match object.get("stream") {
        Some(value) => value
            .as_bool()
            .ok_or_else(|| ProtocolError::InvalidPayload("stream must be a boolean".into()))?,
        None => false,
    };
    if stream && !request.operation.allows_stream() {
        return Err(ProtocolError::InvalidPayload(
            "this operation does not support streaming".into(),
        ));
    }
    let affinity = affinity::extract(request.operation, &request.headers, object)?;
    let thinking_level = extract_thinking_level(object);
    let body_encoding = request_body_encoding(&request.headers)?;

    Ok(DecodedRequest {
        dialect,
        operation: request.operation,
        client_headers: request.headers.clone(),
        headers: HeaderMap::new(),
        body_encoding,
        model: Some(model),
        stream,
        thinking_level,
        affinity,
        payload: AdapterPayload::Json(value),
    })
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

/// Best-effort thinking/reasoning level for request logs.
///
/// Supports common client shapes without failing decode on unknown fields:
/// - `reasoning.effort` (Responses)
/// - `reasoning_effort` (Chat Completions)
/// - `thinking` string or `{ type, budget_tokens }` (Claude-style)
fn extract_thinking_level(object: &Map<String, Value>) -> Option<String> {
    if let Some(effort) = object
        .get("reasoning")
        .and_then(Value::as_object)
        .and_then(|reasoning| reasoning.get("effort"))
        .and_then(Value::as_str)
    {
        return bound_thinking_level(effort);
    }
    if let Some(effort) = object.get("reasoning_effort").and_then(Value::as_str) {
        return bound_thinking_level(effort);
    }
    match object.get("thinking") {
        Some(Value::String(value)) => bound_thinking_level(value),
        Some(Value::Object(thinking)) => {
            let kind = thinking.get("type").and_then(Value::as_str);
            let budget = thinking.get("budget_tokens").and_then(Value::as_u64);
            match (kind, budget) {
                (Some(kind), Some(budget)) => bound_thinking_level(format!("{kind}:{budget}")),
                (Some(kind), None) => bound_thinking_level(kind),
                (None, Some(budget)) => bound_thinking_level(format!("budget:{budget}")),
                (None, None) => None,
            }
        }
        _ => None,
    }
}

pub(crate) fn encode_request(
    operation: ProtocolOperation,
    forwarded: HeaderMap,
    payload: AdapterPayload,
    upstream_model: &str,
) -> Result<EncodedUpstreamRequest, ProtocolError> {
    let AdapterPayload::Json(mut value) = payload else {
        return Err(ProtocolError::InvalidPayload(
            "request body must be JSON".into(),
        ));
    };
    let object = value.as_object_mut().ok_or_else(|| {
        ProtocolError::InvalidPayload("request body must be a JSON object".into())
    })?;
    let stream = object
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    object.insert("model".into(), Value::String(upstream_model.to_owned()));
    if !operation.allows_stream() {
        object.remove("stream");
    }

    let body = serde_json::to_vec(&value)
        .map(Bytes::from)
        .map_err(|_| ProtocolError::InvalidPayload("request JSON could not be encoded".into()))?;
    let mut headers = forwarded;
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
