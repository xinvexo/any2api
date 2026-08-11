use std::net::IpAddr;

use crate::{
    ConfigRevision, CredentialId, ErrorClass, GatewayApiKeyId, OAuthAccountId, ProtocolDialect,
    ProtocolOperation, ProviderEndpointId, ProxyProfileId, RequestId, RequestQuotaCost,
    RetrySafety, RouteTargetId,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestRoutingMode {
    Balanced,
    Bound,
}

impl RequestRoutingMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Balanced => "balanced",
            Self::Bound => "bound",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "balanced" => Some(Self::Balanced),
            "bound" => Some(Self::Bound),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestAttemptFailureScope {
    Unattributed,
    Authentication,
    Credential,
    CredentialModel,
    RouteOperation,
    ExactCandidate,
    EgressPath,
    Proxy,
    Endpoint,
}

impl RequestAttemptFailureScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unattributed => "unattributed",
            Self::Authentication => "authentication",
            Self::Credential => "credential",
            Self::CredentialModel => "credential_model",
            Self::RouteOperation => "route_operation",
            Self::ExactCandidate => "exact_candidate",
            Self::EgressPath => "egress_path",
            Self::Proxy => "proxy",
            Self::Endpoint => "endpoint",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "unattributed" => Some(Self::Unattributed),
            "authentication" => Some(Self::Authentication),
            "credential" => Some(Self::Credential),
            "credential_model" => Some(Self::CredentialModel),
            "route_operation" => Some(Self::RouteOperation),
            "exact_candidate" => Some(Self::ExactCandidate),
            "egress_path" => Some(Self::EgressPath),
            "proxy" => Some(Self::Proxy),
            "endpoint" => Some(Self::Endpoint),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestAttemptRetryDecision {
    Terminal,
    OAuthRefresh,
    RetrySamePath,
    Reselect,
}

impl RequestAttemptRetryDecision {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Terminal => "terminal",
            Self::OAuthRefresh => "oauth_refresh",
            Self::RetrySamePath => "retry_same_path",
            Self::Reselect => "reselect",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "terminal" => Some(Self::Terminal),
            "oauth_refresh" => Some(Self::OAuthRefresh),
            "retry_same_path" => Some(Self::RetrySamePath),
            "reselect" => Some(Self::Reselect),
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
    /// Canonical client address resolved at ingress.
    pub client_ip: IpAddr,
    pub config_revision: ConfigRevision,
    pub gateway_api_key_id: Option<GatewayApiKeyId>,
    pub ingress_protocol: ProtocolDialect,
    pub operation: ProtocolOperation,
    pub public_model: Option<String>,
    /// Optional reasoning/thinking level from the client request body.
    pub thinking_level: Option<String>,
    pub provider_endpoint_id: Option<ProviderEndpointId>,
    pub credential_id: Option<CredentialId>,
    pub oauth_account_id: Option<OAuthAccountId>,
    pub proxy_profile_id: Option<ProxyProfileId>,
    pub status_code: u16,
    pub error_class: Option<ErrorClass>,
    /// any2api-generated safe diagnostic; never an official Provider message.
    pub error_message: Option<String>,
    pub attempt_count: u32,
    pub latency_ms: u64,
    pub first_token_ms: Option<u64>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub quota_cost: Option<RequestQuotaCost>,
    pub is_stream: bool,
}

/// Bounded thinking/reasoning level text for request logs.
pub const MAX_REQUEST_LOG_THINKING_LEVEL_CHARS: usize = 64;

/// Normalize optional thinking/reasoning level from a decoded request body.
#[must_use]
pub fn bound_thinking_level(value: impl AsRef<str>) -> Option<String> {
    let trimmed = value.as_ref().trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut bounded = String::new();
    for character in trimmed.chars() {
        if character.is_control() {
            continue;
        }
        if bounded.chars().count() >= MAX_REQUEST_LOG_THINKING_LEVEL_CHARS {
            break;
        }
        bounded.push(character);
    }
    let bounded = bounded.trim().to_owned();
    (!bounded.is_empty()).then_some(bounded)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestAttempt {
    pub request_id: RequestId,
    pub attempt_no: u32,
    pub route_target_id: Option<RouteTargetId>,
    pub credential_id: Option<CredentialId>,
    pub oauth_account_id: Option<OAuthAccountId>,
    pub proxy_profile_id: Option<ProxyProfileId>,
    /// `None` only for rows created before routing diagnostics were added.
    pub routing_mode: Option<RequestRoutingMode>,
    pub started_at_ms: u64,
    pub duration_ms: u64,
    pub retry_safety: Option<RetrySafety>,
    pub failure_scope: Option<RequestAttemptFailureScope>,
    pub retry_decision: Option<RequestAttemptRetryDecision>,
    pub error_class: Option<ErrorClass>,
    /// Transport/upstream/local diagnostic text for this attempt.
    pub error_message: Option<String>,
    pub status_code: Option<u16>,
    pub outcome: RequestAttemptOutcome,
    pub transport: Option<crate::RequestAttemptTransport>,
    pub stream_timing: Option<crate::RequestAttemptStreamTiming>,
}

/// Normalize and truncate trusted diagnostic text for SQLite/admin display.
#[must_use]
pub fn bound_error_message(message: impl AsRef<str>) -> String {
    let message = message.as_ref().trim();
    if message.is_empty() {
        return String::new();
    }
    let mut bounded = String::new();
    let mut char_count = 0;
    let mut pending_space = false;
    let mut truncated = false;
    for character in message.chars() {
        if character.is_whitespace() || character.is_control() {
            pending_space = !bounded.is_empty();
            continue;
        }
        if pending_space {
            if char_count == MAX_REQUEST_LOG_ERROR_MESSAGE_CHARS {
                truncated = true;
                break;
            }
            bounded.push(' ');
            char_count += 1;
            pending_space = false;
        }
        if char_count == MAX_REQUEST_LOG_ERROR_MESSAGE_CHARS {
            truncated = true;
            break;
        }
        bounded.push(character);
        char_count += 1;
    }
    if truncated {
        if char_count == MAX_REQUEST_LOG_ERROR_MESSAGE_CHARS {
            bounded.pop();
        }
        bounded.push('…');
    }
    bounded
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletedRequestLog {
    pub request: RequestLog,
    pub attempts: Vec<RequestAttempt>,
}

impl CompletedRequestLog {
    /// Bytes exclusively owned by this record, based on current container
    /// capacities rather than request-body limits.
    #[must_use]
    pub fn estimated_owned_bytes(&self) -> usize {
        let request_strings = option_string_capacity(&self.request.public_model)
            .saturating_add(option_string_capacity(&self.request.thinking_level))
            .saturating_add(option_string_capacity(&self.request.error_message))
            .saturating_add(
                self.request
                    .quota_cost
                    .as_ref()
                    .map_or(0, |cost| cost.rate_card.capacity()),
            );
        self.attempts.iter().fold(
            std::mem::size_of::<Self>()
                .saturating_add(request_strings)
                .saturating_add(
                    self.attempts
                        .capacity()
                        .saturating_mul(std::mem::size_of::<RequestAttempt>()),
                ),
            |bytes, attempt| {
                bytes
                    .saturating_add(option_string_capacity(&attempt.error_message))
                    .saturating_add(
                        attempt
                            .transport
                            .as_ref()
                            .map_or(0, |transport| transport.wire_profile_id.capacity()),
                    )
            },
        )
    }
}

fn option_string_capacity(value: &Option<String>) -> usize {
    value.as_ref().map_or(0, String::capacity)
}

#[cfg(test)]
mod tests {
    use super::{MAX_REQUEST_LOG_ERROR_MESSAGE_CHARS, bound_error_message};

    #[test]
    fn bound_error_message_truncates_long_text() {
        let long = "a".repeat(MAX_REQUEST_LOG_ERROR_MESSAGE_CHARS + 20);
        let bounded = bound_error_message(&long);
        assert!(bounded.ends_with('…'));
        assert_eq!(bounded.chars().count(), MAX_REQUEST_LOG_ERROR_MESSAGE_CHARS);
    }

    #[test]
    fn bound_error_message_normalizes_control_whitespace() {
        assert_eq!(
            bound_error_message("  upstream\n\r\tfailed\0 safely  "),
            "upstream failed safely"
        );
    }
}
