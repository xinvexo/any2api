use crate::{
    ConfigRevision, CredentialId, ErrorClass, GatewayApiKeyId, ProtocolDialect, ProtocolOperation,
    ProviderEndpointId, ProxyProfileId, RequestId, RetrySafety, RouteTargetId,
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
    pub proxy_profile_id: Option<ProxyProfileId>,
    pub status_code: u16,
    pub error_class: Option<ErrorClass>,
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
    pub proxy_profile_id: Option<ProxyProfileId>,
    pub started_at_ms: u64,
    pub duration_ms: u64,
    pub retry_safety: Option<RetrySafety>,
    pub error_class: Option<ErrorClass>,
    pub status_code: Option<u16>,
    pub outcome: RequestAttemptOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletedRequestLog {
    pub request: RequestLog,
    pub attempts: Vec<RequestAttempt>,
}
