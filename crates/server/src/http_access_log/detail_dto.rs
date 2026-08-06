use any2api_domain::{HttpAccessLog, HttpAccessLogExchange, HttpBodyCapture, HttpHeader};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Serialize;

use super::dto::SystemLogResponse;

#[derive(Serialize)]
pub(super) struct SystemLogDetailResponse {
    log: SystemLogResponse,
    exchange: Option<HttpExchangeResponse>,
}

impl From<HttpAccessLog> for SystemLogDetailResponse {
    fn from(value: HttpAccessLog) -> Self {
        let log = value.summary().into();
        Self {
            log,
            exchange: value.exchange.map(HttpExchangeResponse::from),
        }
    }
}

#[derive(Serialize)]
struct HttpExchangeResponse {
    request: HttpMessageResponse,
    response: HttpMessageResponse,
}

impl From<HttpAccessLogExchange> for HttpExchangeResponse {
    fn from(value: HttpAccessLogExchange) -> Self {
        Self {
            request: HttpMessageResponse::new(value.request_headers, value.request_body),
            response: HttpMessageResponse::new(value.response_headers, value.response_body),
        }
    }
}

#[derive(Serialize)]
struct HttpMessageResponse {
    headers: Vec<HttpHeaderResponse>,
    body: HttpBodyResponse,
}

impl HttpMessageResponse {
    fn new(headers: Vec<HttpHeader>, body: HttpBodyCapture) -> Self {
        Self {
            headers: headers.into_iter().map(HttpHeaderResponse::from).collect(),
            body: body.into(),
        }
    }
}

#[derive(Serialize)]
struct HttpHeaderResponse {
    name: String,
    value: String,
    encoding: &'static str,
}

impl From<HttpHeader> for HttpHeaderResponse {
    fn from(value: HttpHeader) -> Self {
        let HttpHeader { name, value } = value;
        let (value, encoding) = encode_bytes(value);
        Self {
            name,
            value,
            encoding,
        }
    }
}

#[derive(Serialize)]
struct HttpBodyResponse {
    content: String,
    encoding: &'static str,
    captured_bytes: usize,
    total_bytes: u64,
    complete: bool,
    truncated: bool,
}

impl From<HttpBodyCapture> for HttpBodyResponse {
    fn from(value: HttpBodyCapture) -> Self {
        let captured_bytes = value.captured_bytes();
        let total_bytes = value.total_bytes();
        let complete = value.is_complete();
        let truncated = value.is_truncated();
        let (content, encoding) = encode_bytes(value.into_content());
        Self {
            content,
            encoding,
            captured_bytes,
            total_bytes,
            complete,
            truncated,
        }
    }
}

fn encode_bytes(value: impl AsRef<[u8]>) -> (String, &'static str) {
    match std::str::from_utf8(value.as_ref()) {
        Ok(value) => (value.to_owned(), "utf8"),
        Err(_) => (STANDARD.encode(value.as_ref()), "base64"),
    }
}

#[cfg(test)]
mod tests {
    use any2api_domain::{ConfigRevision, HttpAccessLogOutcome, HttpProtocolVersion, RequestId};

    use super::*;

    #[test]
    fn detail_keeps_repeated_headers_and_base64_encodes_binary_values() {
        let response = SystemLogDetailResponse::from(HttpAccessLog {
            request_id: RequestId::new(),
            started_at_ms: 1,
            config_revision: ConfigRevision::INITIAL,
            client_ip: None,
            method: "POST".to_owned(),
            path: "/v1/responses".to_owned(),
            uri: "/v1/responses?raw=yes".to_owned(),
            http_version: HttpProtocolVersion::Http11,
            status_code: Some(200),
            duration_ms: 1,
            response_bytes: 2,
            outcome: HttpAccessLogOutcome::Completed,
            gateway_auth_rejected: false,
            exchange: Some(HttpAccessLogExchange {
                request_headers: vec![
                    HttpHeader {
                        name: "x-value".to_owned(),
                        value: b"one".to_vec(),
                    },
                    HttpHeader {
                        name: "x-value".to_owned(),
                        value: vec![0xff, 0x00],
                    },
                ],
                request_body: HttpBodyCapture::from_vec(
                    b"{\"prompt\":\"raw\"}".to_vec(),
                    16,
                    true,
                    false,
                ),
                response_headers: Vec::new(),
                response_body: HttpBodyCapture::from_vec(b"ok".to_vec(), 2, true, false),
            }),
        });

        let json = serde_json::to_value(response).expect("detail JSON");
        assert_eq!(json["log"]["uri"], "/v1/responses?raw=yes");
        assert_eq!(json["exchange"]["request"]["headers"][0]["value"], "one");
        assert_eq!(
            json["exchange"]["request"]["headers"][1]["encoding"],
            "base64"
        );
        assert_eq!(
            json["exchange"]["request"]["body"]["content"],
            "{\"prompt\":\"raw\"}"
        );
    }
}
