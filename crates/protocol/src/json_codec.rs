use any2api_domain::{ProtocolDialect, ProtocolOperation};
use bytes::Bytes;
use http::{HeaderMap, HeaderValue, Method, Uri, header};
use serde_json::Value;

use crate::{
    ProtocolError,
    api::{AdapterPayload, DecodedRequest, EncodedUpstreamRequest, IngressRequest},
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

    Ok(DecodedRequest {
        dialect,
        operation: request.operation,
        headers: forwarded_headers(&request.headers, dialect),
        model: Some(model),
        stream,
        payload: AdapterPayload::RawJson(request.body),
    })
}

pub(crate) fn encode_request(
    operation: ProtocolOperation,
    forwarded: HeaderMap,
    payload: AdapterPayload,
    upstream_model: &str,
) -> Result<EncodedUpstreamRequest, ProtocolError> {
    let AdapterPayload::RawJson(body) = payload;
    let mut value: Value = serde_json::from_slice(&body)
        .map_err(|_| ProtocolError::InvalidPayload("request body must be valid JSON".into()))?;
    let object = value.as_object_mut().ok_or_else(|| {
        ProtocolError::InvalidPayload("request body must be a JSON object".into())
    })?;
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
    headers.insert(header::ACCEPT, HeaderValue::from_static("application/json"));

    Ok(EncodedUpstreamRequest {
        method: Method::POST,
        uri: Uri::from_static("/"),
        headers,
        body,
    })
}

fn forwarded_headers(headers: &HeaderMap, dialect: ProtocolDialect) -> HeaderMap {
    let mut forwarded = HeaderMap::new();
    if dialect == ProtocolDialect::AnthropicMessages {
        for value in headers.get_all("anthropic-beta").iter() {
            forwarded.append("anthropic-beta", value.clone());
        }
    }
    forwarded
}
