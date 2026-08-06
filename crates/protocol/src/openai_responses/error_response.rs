use any2api_domain::{PublicError, PublicErrorCode};
use bytes::Bytes;
use http::{HeaderMap, HeaderValue, StatusCode, header};
use serde_json::json;

use crate::api::EgressResponse;

pub(super) fn encode(error: &PublicError) -> EgressResponse {
    let mut response = json_response(
        public_error_status(error.code()),
        json!({
            "error": {
                "message": error.client_message(),
                "type": error_type(error.code()),
                "param": error_param(error.code()),
                "code": error_code(error.code())
            }
        }),
    );
    insert_retry_after(&mut response.headers, error.retry_after_seconds());
    response
}

fn error_type(code: PublicErrorCode) -> &'static str {
    match code {
        PublicErrorCode::Unauthorized => "authentication_error",
        PublicErrorCode::InvalidRequest
        | PublicErrorCode::PayloadTooLarge
        | PublicErrorCode::PublicApiNotFound
        | PublicErrorCode::MethodNotAllowed
        | PublicErrorCode::ModelNotFound
        | PublicErrorCode::SessionBindingLost => "invalid_request_error",
        PublicErrorCode::NoAvailableCredential | PublicErrorCode::LocalRateLimit => {
            "rate_limit_error"
        }
        PublicErrorCode::UpstreamError
        | PublicErrorCode::GatewayTimeout
        | PublicErrorCode::InternalError => "server_error",
    }
}

fn error_code(code: PublicErrorCode) -> &'static str {
    match code {
        PublicErrorCode::Unauthorized => "unauthorized",
        PublicErrorCode::InvalidRequest => "invalid_request",
        PublicErrorCode::PayloadTooLarge => "payload_too_large",
        PublicErrorCode::PublicApiNotFound => "public_api_not_found",
        PublicErrorCode::MethodNotAllowed => "method_not_allowed",
        PublicErrorCode::ModelNotFound => "model_not_found",
        PublicErrorCode::NoAvailableCredential => "no_available_credential",
        PublicErrorCode::LocalRateLimit => "local_rate_limit",
        PublicErrorCode::SessionBindingLost => "session_binding_lost",
        PublicErrorCode::UpstreamError => "upstream_error",
        PublicErrorCode::GatewayTimeout => "gateway_timeout",
        PublicErrorCode::InternalError => "internal_error",
    }
}

fn error_param(code: PublicErrorCode) -> Option<&'static str> {
    (code == PublicErrorCode::ModelNotFound).then_some("model")
}

fn public_error_status(code: PublicErrorCode) -> StatusCode {
    match code {
        PublicErrorCode::Unauthorized => StatusCode::UNAUTHORIZED,
        PublicErrorCode::InvalidRequest => StatusCode::BAD_REQUEST,
        PublicErrorCode::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
        PublicErrorCode::PublicApiNotFound => StatusCode::NOT_FOUND,
        PublicErrorCode::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
        PublicErrorCode::ModelNotFound => StatusCode::BAD_REQUEST,
        PublicErrorCode::NoAvailableCredential | PublicErrorCode::LocalRateLimit => {
            StatusCode::TOO_MANY_REQUESTS
        }
        PublicErrorCode::SessionBindingLost => StatusCode::CONFLICT,
        PublicErrorCode::UpstreamError => StatusCode::BAD_GATEWAY,
        PublicErrorCode::GatewayTimeout => StatusCode::GATEWAY_TIMEOUT,
        PublicErrorCode::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn json_response(status: StatusCode, value: serde_json::Value) -> EgressResponse {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    EgressResponse {
        status,
        headers,
        body: Bytes::from(serde_json::to_vec(&value).expect("JSON value encodes")),
    }
}

fn insert_retry_after(headers: &mut HeaderMap, seconds: Option<u64>) {
    if let Some(seconds) = seconds
        && let Ok(value) = HeaderValue::from_str(&seconds.to_string())
    {
        headers.insert(header::RETRY_AFTER, value);
    }
}
