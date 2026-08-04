use std::{fmt, net::IpAddr};

use serde::{Deserialize, Serialize};

use crate::{ConfigRevision, RequestId};

/// A request or response body may be unbounded (for example SSE), so access
/// logging retains a prefix without changing downstream backpressure.
pub const MAX_HTTP_ACCESS_LOG_BODY_CAPTURE_BYTES: usize = 1024 * 1024;

/// Gateway authentication rejections may use at most this share of bounded
/// telemetry capacity, leaving normal request history at higher priority.
pub const GATEWAY_AUTH_REJECTED_CAPACITY_DIVISOR: u64 = 4;

#[must_use]
pub const fn gateway_auth_rejected_capacity(total: u64) -> u64 {
    let capacity = total / GATEWAY_AUTH_REJECTED_CAPACITY_DIVISOR;
    if capacity == 0 { 1 } else { capacity }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpAccessLogOutcome {
    Completed,
    BodyError,
    Cancelled,
}

impl HttpAccessLogOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::BodyError => "body_error",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "completed" => Some(Self::Completed),
            "body_error" => Some(Self::BodyError),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpProtocolVersion {
    Http09,
    Http10,
    Http11,
    Http2,
    Http3,
}

impl HttpProtocolVersion {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Http09 => "HTTP/0.9",
            Self::Http10 => "HTTP/1.0",
            Self::Http11 => "HTTP/1.1",
            Self::Http2 => "HTTP/2",
            Self::Http3 => "HTTP/3",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "HTTP/0.9" => Some(Self::Http09),
            "HTTP/1.0" => Some(Self::Http10),
            "HTTP/1.1" => Some(Self::Http11),
            "HTTP/2" => Some(Self::Http2),
            "HTTP/3" => Some(Self::Http3),
            _ => None,
        }
    }
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct HttpHeader {
    pub name: String,
    pub value: Vec<u8>,
}

impl fmt::Debug for HttpHeader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpHeader")
            .field("name", &self.name)
            .field("value_bytes", &self.value.len())
            .finish()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct HttpBodyCapture {
    pub content: Vec<u8>,
    /// Bytes observed from the body, including bytes beyond the captured prefix.
    pub total_bytes: u64,
    /// True only when the body reached a normal EOF.
    pub complete: bool,
    pub truncated: bool,
}

impl fmt::Debug for HttpBodyCapture {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpBodyCapture")
            .field("captured_bytes", &self.content.len())
            .field("total_bytes", &self.total_bytes)
            .field("complete", &self.complete)
            .field("truncated", &self.truncated)
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpAccessLogExchange {
    pub request_headers: Vec<HttpHeader>,
    pub request_body: HttpBodyCapture,
    pub response_headers: Vec<HttpHeader>,
    pub response_body: HttpBodyCapture,
}

#[derive(Clone, Eq, PartialEq)]
pub struct HttpAccessLog {
    pub request_id: RequestId,
    pub started_at_ms: u64,
    pub config_revision: ConfigRevision,
    pub client_ip: Option<IpAddr>,
    pub method: String,
    /// Exact request URI path received by Axum, used by retention policy.
    pub path: String,
    /// Complete URI received by Axum, including any query string.
    pub uri: String,
    pub http_version: HttpProtocolVersion,
    pub status_code: Option<u16>,
    pub duration_ms: u64,
    pub response_bytes: u64,
    pub outcome: HttpAccessLogOutcome,
    /// True only when the public Gateway authentication layer rejected the
    /// request before establishing an authenticated Gateway API key.
    pub gateway_auth_rejected: bool,
    /// `None` identifies a row created before raw exchange capture existed.
    pub exchange: Option<HttpAccessLogExchange>,
}

impl HttpAccessLog {
    #[must_use]
    pub fn summary(&self) -> HttpAccessLogSummary {
        HttpAccessLogSummary {
            request_id: self.request_id,
            started_at_ms: self.started_at_ms,
            config_revision: self.config_revision,
            client_ip: self.client_ip,
            method: self.method.clone(),
            path: self.path.clone(),
            uri: self.uri.clone(),
            http_version: self.http_version,
            status_code: self.status_code,
            duration_ms: self.duration_ms,
            response_bytes: self.response_bytes,
            outcome: self.outcome,
            exchange_captured: self.exchange.is_some(),
        }
    }
}

impl fmt::Debug for HttpAccessLog {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpAccessLog")
            .field("request_id", &self.request_id)
            .field("started_at_ms", &self.started_at_ms)
            .field("method", &self.method)
            .field("path", &self.path)
            .field("status_code", &self.status_code)
            .field("outcome", &self.outcome)
            .field("gateway_auth_rejected", &self.gateway_auth_rejected)
            .field("exchange_captured", &self.exchange.is_some())
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct HttpAccessLogSummary {
    pub request_id: RequestId,
    pub started_at_ms: u64,
    pub config_revision: ConfigRevision,
    pub client_ip: Option<IpAddr>,
    pub method: String,
    pub path: String,
    pub uri: String,
    pub http_version: HttpProtocolVersion,
    pub status_code: Option<u16>,
    pub duration_ms: u64,
    pub response_bytes: u64,
    pub outcome: HttpAccessLogOutcome,
    pub exchange_captured: bool,
}

impl fmt::Debug for HttpAccessLogSummary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpAccessLogSummary")
            .field("request_id", &self.request_id)
            .field("started_at_ms", &self.started_at_ms)
            .field("method", &self.method)
            .field("path", &self.path)
            .field("status_code", &self.status_code)
            .field("outcome", &self.outcome)
            .field("exchange_captured", &self.exchange_captured)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_marks_exchange_availability_without_copying_raw_values() {
        let log = HttpAccessLog {
            request_id: RequestId::new(),
            started_at_ms: 1,
            config_revision: ConfigRevision::INITIAL,
            client_ip: None,
            method: "POST".to_owned(),
            path: "/v1/responses".to_owned(),
            uri: "/v1/responses?trace=raw".to_owned(),
            http_version: HttpProtocolVersion::Http11,
            status_code: Some(200),
            duration_ms: 2,
            response_bytes: 2,
            outcome: HttpAccessLogOutcome::Completed,
            gateway_auth_rejected: false,
            exchange: Some(HttpAccessLogExchange {
                request_headers: vec![HttpHeader {
                    name: "authorization".to_owned(),
                    value: b"secret".to_vec(),
                }],
                request_body: HttpBodyCapture {
                    content: b"{}".to_vec(),
                    total_bytes: 2,
                    complete: true,
                    truncated: false,
                },
                response_headers: Vec::new(),
                response_body: HttpBodyCapture {
                    content: b"ok".to_vec(),
                    total_bytes: 2,
                    complete: true,
                    truncated: false,
                },
            }),
        };

        let summary = log.summary();
        assert_eq!(summary.uri, "/v1/responses?trace=raw");
        assert!(summary.exchange_captured);
        assert!(!format!("{log:?}").contains("secret"));
    }

    #[test]
    fn gateway_auth_rejection_capacity_is_one_quarter_with_a_small_floor() {
        assert_eq!(gateway_auth_rejected_capacity(1), 1);
        assert_eq!(gateway_auth_rejected_capacity(4), 1);
        assert_eq!(gateway_auth_rejected_capacity(20), 5);
    }
}
