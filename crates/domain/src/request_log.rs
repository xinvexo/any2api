use crate::{
    ConfigRevision, CredentialId, ErrorClass, GatewayApiKeyId, OAuthAccountId, ProtocolDialect,
    ProtocolOperation, ProviderEndpointId, ProxyProfileId, RequestId, RetrySafety, RouteTargetId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestAttemptOutcome {
    Success,
    TransportError,
    UpstreamError,
    InvalidResponse,
    LocalError,
    StreamError,
    Cancelled,
}

impl RequestAttemptOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::TransportError => "transport_error",
            Self::UpstreamError => "upstream_error",
            Self::InvalidResponse => "invalid_response",
            Self::LocalError => "local_error",
            Self::StreamError => "stream_error",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "success" => Some(Self::Success),
            "transport_error" => Some(Self::TransportError),
            "upstream_error" => Some(Self::UpstreamError),
            "invalid_response" => Some(Self::InvalidResponse),
            "local_error" => Some(Self::LocalError),
            "stream_error" => Some(Self::StreamError),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

/// Bounded text for admin diagnostics. Not full request/response bodies.
pub const MAX_REQUEST_LOG_ERROR_MESSAGE_CHARS: usize = 1_024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestLog {
    pub request_id: RequestId,
    pub started_at_ms: u64,
    pub config_revision: ConfigRevision,
    pub gateway_api_key_id: Option<GatewayApiKeyId>,
    pub ingress_protocol: ProtocolDialect,
    pub operation: ProtocolOperation,
    pub public_model: Option<String>,
    pub provider_endpoint_id: Option<ProviderEndpointId>,
    pub credential_id: Option<CredentialId>,
    pub oauth_account_id: Option<OAuthAccountId>,
    pub proxy_profile_id: Option<ProxyProfileId>,
    pub status_code: u16,
    pub error_class: Option<ErrorClass>,
    /// Client-visible public error message, or best attempt/transport summary.
    pub error_message: Option<String>,
    pub attempt_count: u32,
    pub latency_ms: u64,
    pub first_token_ms: Option<u64>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub cache_write_tokens: Option<u64>,
    pub is_stream: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestAttempt {
    pub request_id: RequestId,
    pub attempt_no: u32,
    pub route_target_id: Option<RouteTargetId>,
    pub credential_id: Option<CredentialId>,
    pub oauth_account_id: Option<OAuthAccountId>,
    pub proxy_profile_id: Option<ProxyProfileId>,
    pub started_at_ms: u64,
    pub duration_ms: u64,
    pub retry_safety: Option<RetrySafety>,
    pub error_class: Option<ErrorClass>,
    /// Transport/upstream/local diagnostic text for this attempt.
    pub error_message: Option<String>,
    pub status_code: Option<u16>,
    pub outcome: RequestAttemptOutcome,
}

/// Truncate diagnostic text for SQLite/admin display without storing full bodies.
#[must_use]
pub fn bound_error_message(message: impl AsRef<str>) -> String {
    let message = message.as_ref().trim();
    if message.is_empty() {
        return String::new();
    }
    let mut end = message.len().min(MAX_REQUEST_LOG_ERROR_MESSAGE_CHARS);
    while end > 0 && !message.is_char_boundary(end) {
        end -= 1;
    }
    let mut bounded = message[..end].to_owned();
    if end < message.len() {
        bounded.push('…');
    }
    bounded
}

/// Prefer structured upstream error fields; fall back to bounded UTF-8 body text.
#[must_use]
pub fn extract_upstream_error_message(body: &[u8]) -> Option<String> {
    if body.is_empty() {
        return None;
    }
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) {
        if let Some(message) = json_error_message(&value) {
            let bounded = bound_error_message(message);
            if !bounded.is_empty() {
                return Some(bounded);
            }
        }
    }
    let lossy = String::from_utf8_lossy(body);
    let bounded = bound_error_message(lossy.as_ref());
    if bounded.is_empty() {
        None
    } else {
        Some(bounded)
    }
}

fn json_error_message(value: &serde_json::Value) -> Option<&str> {
    let object = value.as_object()?;
    if let Some(message) = object.get("message").and_then(serde_json::Value::as_str) {
        return Some(message);
    }
    if let Some(error) = object.get("error") {
        if let Some(message) = error.get("message").and_then(serde_json::Value::as_str) {
            return Some(message);
        }
        if let Some(message) = error.as_str() {
            return Some(message);
        }
        if let Some(error_object) = error.as_object() {
            let kind = error_object
                .get("type")
                .or_else(|| error_object.get("code"))
                .and_then(serde_json::Value::as_str);
            if let Some(kind) = kind {
                return Some(kind);
            }
        }
    }
    None
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletedRequestLog {
    pub request: RequestLog,
    pub attempts: Vec<RequestAttempt>,
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_REQUEST_LOG_ERROR_MESSAGE_CHARS, bound_error_message, extract_upstream_error_message,
    };

    #[test]
    fn bound_error_message_truncates_long_text() {
        let long = "a".repeat(MAX_REQUEST_LOG_ERROR_MESSAGE_CHARS + 20);
        let bounded = bound_error_message(&long);
        assert!(bounded.ends_with('…'));
        assert!(bounded.chars().count() <= MAX_REQUEST_LOG_ERROR_MESSAGE_CHARS + 1);
    }

    #[test]
    fn extract_prefers_structured_error_message() {
        let body =
            br#"{"error":{"message":"Incorrect API key provided","type":"invalid_request_error"}}"#;
        assert_eq!(
            extract_upstream_error_message(body).as_deref(),
            Some("Incorrect API key provided")
        );
    }

    #[test]
    fn extract_falls_back_to_plain_text() {
        assert_eq!(
            extract_upstream_error_message(b"upstream blew up").as_deref(),
            Some("upstream blew up")
        );
    }
}
