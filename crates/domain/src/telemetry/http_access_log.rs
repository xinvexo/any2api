use std::{fmt, net::IpAddr};

use crate::{ConfigRevision, RequestId};

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

#[derive(Eq, PartialEq)]
pub struct HttpAccessLog {
    pub request_id: RequestId,
    pub started_at_ms: u64,
    pub config_revision: ConfigRevision,
    pub client_ip: Option<IpAddr>,
    pub method: String,
    /// Exact request URI path received by Axum, used by retention policy.
    pub path: String,
    pub http_version: HttpProtocolVersion,
    pub status_code: Option<u16>,
    pub duration_ms: u64,
    pub response_bytes: u64,
    pub outcome: HttpAccessLogOutcome,
    /// True only when the public Gateway authentication layer rejected the
    /// request before establishing an authenticated Gateway API key.
    pub gateway_auth_rejected: bool,
}

impl HttpAccessLog {
    /// Bytes exclusively owned by this record, based on current container
    /// capacities rather than protocol maximums.
    #[must_use]
    pub fn estimated_owned_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.method.capacity())
            .saturating_add(self.path.capacity())
    }

    #[must_use]
    pub fn summary(&self) -> HttpAccessLogSummary {
        HttpAccessLogSummary {
            request_id: self.request_id,
            started_at_ms: self.started_at_ms,
            config_revision: self.config_revision,
            client_ip: self.client_ip,
            method: self.method.clone(),
            path: self.path.clone(),
            http_version: self.http_version,
            status_code: self.status_code,
            duration_ms: self.duration_ms,
            response_bytes: self.response_bytes,
            outcome: self.outcome,
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
    pub http_version: HttpProtocolVersion,
    pub status_code: Option<u16>,
    pub duration_ms: u64,
    pub response_bytes: u64,
    pub outcome: HttpAccessLogOutcome,
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
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_is_metadata_only() {
        let log = HttpAccessLog {
            request_id: RequestId::new(),
            started_at_ms: 1,
            config_revision: ConfigRevision::INITIAL,
            client_ip: None,
            method: "POST".to_owned(),
            path: "/v1/responses".to_owned(),
            http_version: HttpProtocolVersion::Http11,
            status_code: Some(200),
            duration_ms: 2,
            response_bytes: 2,
            outcome: HttpAccessLogOutcome::Completed,
            gateway_auth_rejected: false,
        };

        let summary = log.summary();
        assert_eq!(summary.path, "/v1/responses");
    }

    #[test]
    fn gateway_auth_rejection_capacity_is_one_quarter_with_a_small_floor() {
        assert_eq!(gateway_auth_rejected_capacity(1), 1);
        assert_eq!(gateway_auth_rejected_capacity(4), 1);
        assert_eq!(gateway_auth_rejected_capacity(20), 5);
    }

    #[test]
    fn owned_byte_estimate_uses_allocated_container_capacities() {
        let mut log = HttpAccessLog {
            request_id: RequestId::new(),
            started_at_ms: 1,
            config_revision: ConfigRevision::INITIAL,
            client_ip: None,
            method: String::with_capacity(32),
            path: String::with_capacity(64),
            http_version: HttpProtocolVersion::Http11,
            status_code: Some(200),
            duration_ms: 1,
            response_bytes: 0,
            outcome: HttpAccessLogOutcome::Completed,
            gateway_auth_rejected: false,
        };
        log.method.push_str("POST");
        log.path.push_str("/v1/responses");

        let expected = std::mem::size_of::<HttpAccessLog>() + 32 + 64;
        assert_eq!(log.estimated_owned_bytes(), expected);
    }
}
