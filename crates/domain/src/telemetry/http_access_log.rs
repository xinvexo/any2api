use std::net::IpAddr;

use crate::{ConfigRevision, RequestId};

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

/// Storage enforces `length(method) BETWEEN 1 AND 32`; clients may send
/// arbitrarily long extension methods, so collection must truncate first or
/// one hostile request poisons a whole telemetry write batch.
pub const MAX_HTTP_ACCESS_LOG_METHOD_CHARS: usize = 32;
/// Request lines can carry paths of hundreds of kilobytes; bound what a
/// single log row may retain.
pub const MAX_HTTP_ACCESS_LOG_PATH_CHARS: usize = 1_024;

/// Truncate to at most `max_chars` characters on a character boundary.
#[must_use]
pub fn bounded_log_text(value: &str, max_chars: usize) -> &str {
    match value.char_indices().nth(max_chars) {
        Some((offset, _)) => &value[..offset],
        None => value,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpAccessLog {
    pub request_id: RequestId,
    pub started_at_ms: u64,
    pub config_revision: ConfigRevision,
    pub client_ip: Option<IpAddr>,
    pub method: String,
    /// The request URI path received by the server, truncated to
    /// [`MAX_HTTP_ACCESS_LOG_PATH_CHARS`]. Query parameters are excluded.
    pub path: String,
    pub http_version: HttpProtocolVersion,
    pub status_code: Option<u16>,
    pub duration_ms: u64,
    pub response_bytes: u64,
    pub outcome: HttpAccessLogOutcome,
}

#[cfg(test)]
mod tests {
    use super::{MAX_HTTP_ACCESS_LOG_METHOD_CHARS, bounded_log_text};

    #[test]
    fn bounded_log_text_truncates_on_character_boundaries() {
        assert_eq!(
            bounded_log_text("GET", MAX_HTTP_ACCESS_LOG_METHOD_CHARS),
            "GET"
        );
        let long = "M".repeat(MAX_HTTP_ACCESS_LOG_METHOD_CHARS + 5);
        assert_eq!(
            bounded_log_text(&long, MAX_HTTP_ACCESS_LOG_METHOD_CHARS)
                .chars()
                .count(),
            MAX_HTTP_ACCESS_LOG_METHOD_CHARS
        );
        assert_eq!(bounded_log_text("héllo", 2), "hé");
    }
}
