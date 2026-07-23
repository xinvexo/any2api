use std::fmt;

use any2api_domain::{ProtocolDialect, ProtocolOperation, PublicError, TokenUsage};
use bytes::Bytes;
use http::{HeaderMap, Method, StatusCode, Uri};

pub use crate::{ProtocolError, ProtocolRegistry};

mod exchange;

pub use exchange::{PreparedProtocolRequest, ProtocolExchange, StartedProtocolBridge};

#[derive(Clone)]
pub struct IngressRequest {
    pub method: Method,
    pub uri: Uri,
    pub headers: HeaderMap,
    pub body: Bytes,
    pub operation: ProtocolOperation,
}

#[derive(Clone)]
pub struct DecodedRequest {
    pub dialect: ProtocolDialect,
    pub operation: ProtocolOperation,
    pub headers: HeaderMap,
    pub model: Option<String>,
    pub stream: bool,
    pub affinity: IngressAffinity,
    pub payload: AdapterPayload,
}

#[derive(Clone, Eq, PartialEq)]
pub enum IngressAffinity {
    None,
    Hard(String),
    Soft(String),
}

#[derive(Clone)]
pub enum AdapterPayload {
    RawJson(Bytes),
}

#[derive(Clone)]
pub struct EncodedUpstreamRequest {
    pub method: Method,
    pub uri: Uri,
    pub headers: HeaderMap,
    pub body: Bytes,
}

#[derive(Clone)]
pub struct UpstreamResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
}

#[derive(Clone)]
pub struct DecodedUpstreamResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub payload: AdapterPayload,
    pub telemetry: ProtocolResponseTelemetry,
}

#[derive(Clone)]
pub struct EgressResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
}

#[derive(Clone, Eq, PartialEq)]
pub struct SseFrame(pub Bytes);

#[derive(Clone, Eq, PartialEq)]
pub struct AdapterEvent {
    bytes: Bytes,
    telemetry: ProtocolEventTelemetry,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProtocolResponseTelemetry {
    pub token_usage: TokenUsage,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProtocolEventTelemetry {
    pub token_usage: TokenUsage,
    pub has_content_delta: bool,
}

impl AdapterEvent {
    #[must_use]
    pub fn new(bytes: Bytes, telemetry: ProtocolEventTelemetry) -> Self {
        Self { bytes, telemetry }
    }

    #[must_use]
    pub fn bytes(&self) -> &Bytes {
        &self.bytes
    }

    #[must_use]
    pub const fn telemetry(&self) -> ProtocolEventTelemetry {
        self.telemetry
    }

    #[must_use]
    pub fn into_bytes(self) -> Bytes {
        self.bytes
    }
}

impl fmt::Debug for SseFrame {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SseFrame")
            .field("bytes", &self.0.len())
            .finish()
    }
}

impl fmt::Debug for AdapterEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdapterEvent")
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

pub trait ProtocolAdapter: Send + Sync {
    fn dialect(&self) -> ProtocolDialect;

    fn decode_ingress_request(
        &self,
        request: IngressRequest,
    ) -> Result<DecodedRequest, ProtocolError>;

    fn encode_upstream_request(
        &self,
        operation: ProtocolOperation,
        headers: HeaderMap,
        payload: AdapterPayload,
        upstream_model: &str,
    ) -> Result<EncodedUpstreamRequest, ProtocolError>;

    fn decode_upstream_response(
        &self,
        response: UpstreamResponse,
    ) -> Result<DecodedUpstreamResponse, ProtocolError>;

    fn decode_upstream_event(&self, frame: SseFrame) -> Result<AdapterEvent, ProtocolError>;

    fn encode_egress_response(
        &self,
        response: DecodedUpstreamResponse,
    ) -> Result<EgressResponse, ProtocolError>;

    fn encode_egress_event(
        &self,
        event: AdapterEvent,
        public_model: &str,
    ) -> Result<SseFrame, ProtocolError>;

    fn hard_affinity_id_from_response(
        &self,
        _operation: ProtocolOperation,
        _response: &DecodedUpstreamResponse,
    ) -> Result<Option<String>, ProtocolError> {
        Ok(None)
    }

    fn hard_affinity_id_from_event(
        &self,
        _operation: ProtocolOperation,
        _event: &AdapterEvent,
    ) -> Result<Option<String>, ProtocolError> {
        Ok(None)
    }

    fn error_response(&self, error: &PublicError) -> EgressResponse;
}

pub trait ProtocolBridge: Send + Sync {
    fn ingress_dialect(&self) -> ProtocolDialect;

    fn upstream_dialect(&self) -> ProtocolDialect;

    fn supports_operation(&self, operation: ProtocolOperation) -> bool;

    fn start(
        &self,
        request: DecodedRequest,
        upstream_model: &str,
    ) -> Result<StartedProtocolBridge, ProtocolError>;
}

pub trait ProtocolBridgeSession: Send {
    fn transform_response(
        &mut self,
        response: DecodedUpstreamResponse,
    ) -> Result<DecodedUpstreamResponse, ProtocolError>;

    fn transform_event(&mut self, event: AdapterEvent) -> Result<Vec<AdapterEvent>, ProtocolError>;

    fn finish_events(&mut self) -> Result<Vec<AdapterEvent>, ProtocolError> {
        Ok(Vec::new())
    }
}

impl fmt::Debug for IngressRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IngressRequest")
            .field("method", &self.method)
            .field("uri", &self.uri)
            .field("header_count", &self.headers.len())
            .field("body_bytes", &self.body.len())
            .field("operation", &self.operation)
            .finish()
    }
}

impl fmt::Debug for DecodedRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DecodedRequest")
            .field("dialect", &self.dialect)
            .field("operation", &self.operation)
            .field("model", &self.model)
            .field("stream", &self.stream)
            .field("affinity", &self.affinity)
            .field("payload", &self.payload)
            .finish()
    }
}

impl fmt::Debug for IngressAffinity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::None => "None",
            Self::Hard(_) => "Hard([REDACTED])",
            Self::Soft(_) => "Soft([REDACTED])",
        })
    }
}

impl fmt::Debug for AdapterPayload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RawJson(body) => formatter
                .debug_struct("RawJson")
                .field("body_bytes", &body.len())
                .finish(),
        }
    }
}

macro_rules! impl_redacted_http_debug {
    ($type:ty, $name:literal) => {
        impl fmt::Debug for $type {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_struct($name)
                    .field("status", &self.status)
                    .field("header_count", &self.headers.len())
                    .field("body_bytes", &self.body.len())
                    .finish()
            }
        }
    };
}

impl fmt::Debug for EncodedUpstreamRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EncodedUpstreamRequest")
            .field("method", &self.method)
            .field("uri", &self.uri)
            .field("header_count", &self.headers.len())
            .field("body_bytes", &self.body.len())
            .finish()
    }
}

impl_redacted_http_debug!(UpstreamResponse, "UpstreamResponse");

impl fmt::Debug for DecodedUpstreamResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DecodedUpstreamResponse")
            .field("status", &self.status)
            .field("header_count", &self.headers.len())
            .field("payload", &self.payload)
            .finish()
    }
}

impl_redacted_http_debug!(EgressResponse, "EgressResponse");
