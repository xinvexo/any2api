use std::{borrow::Cow, fmt};

use any2api_domain::{
    ProtocolDialect, ProtocolOperation, PublicError, RequestBodyEncoding, TokenUsage,
};
use async_trait::async_trait;
use bytes::Bytes;
use http::{HeaderMap, Method, StatusCode, Uri};
use serde_json::Value;

pub use crate::{ProtocolError, ProtocolRegistry};

use super::{
    continuation::BridgeContinuationState, exchange::StartedProtocolBridge,
    execution_profile::RequestExecutionProfile,
};
pub use crate::raw_json::RawJsonPayload;
pub use crate::sse::SseJsonData;

#[derive(Clone)]
pub struct IngressRequest {
    pub method: Method,
    pub uri: Uri,
    pub headers: HeaderMap,
    pub body: Bytes,
    pub operation: ProtocolOperation,
}

pub struct DecodedRequest {
    pub dialect: ProtocolDialect,
    pub operation: ProtocolOperation,
    pub execution_profile: RequestExecutionProfile,
    /// Auth-stripped ingress headers retained only for the selected Provider's
    /// bounded allowlist projection. Never log or forward this map wholesale.
    pub client_headers: HeaderMap,
    pub headers: HeaderMap,
    pub body_encoding: RequestBodyEncoding,
    pub model: Option<String>,
    pub stream: bool,
    /// Optional thinking/reasoning level extracted from the request body.
    pub thinking_level: Option<String>,
    pub affinity: IngressAffinity,
    pub payload: AdapterPayload,
}

#[derive(Clone, Eq, PartialEq)]
pub enum IngressAffinity {
    None,
    /// A stateful continuation that must resolve to an existing binding.
    Continuation(String),
    /// An explicit session that may create a binding when one does not exist.
    Session(String),
}

/// Validated request payload. Same-protocol JSON keeps shared wire bytes;
/// bridges materialize a structured value only when conversion needs it.
#[derive(Clone)]
pub enum AdapterPayload {
    Json(Value),
    RawJson(RawJsonPayload),
    Multipart(MultipartPayload),
}

impl AdapterPayload {
    pub(crate) fn materialize_json(&self) -> Result<Cow<'_, Value>, crate::ProtocolError> {
        match self {
            Self::Json(value) => Ok(Cow::Borrowed(value)),
            Self::RawJson(value) => value.to_value().map(Cow::Owned),
            Self::Multipart(_) => Err(crate::ProtocolError::InvalidPayload(
                "request body must be JSON".into(),
            )),
        }
    }
}

/// A parsed multipart body whose ordered fields can be safely re-encoded
/// after replacing routing-owned values such as `model`.
#[derive(Clone)]
pub struct MultipartPayload {
    pub(crate) parts: Vec<MultipartPart>,
    pub(crate) model_part_index: usize,
    pub(crate) model: String,
    pub(crate) stream: bool,
}

#[derive(Clone)]
pub struct MultipartPart {
    pub(crate) name: String,
    pub(crate) headers: HeaderMap,
    pub(crate) body: Bytes,
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
    /// Original wire bytes; `None` once a bridge transform rewrote `parsed`.
    pub body: Option<Bytes>,
    /// JSON body parsed once at decode; telemetry, continuation-ID extraction,
    /// and egress model rewriting all consume this shared parse.
    pub parsed: Value,
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

#[derive(Clone, PartialEq)]
pub struct AdapterEvent {
    bytes: Bytes,
    telemetry: ProtocolEventTelemetry,
    payload: SseEventPayload,
    termination: StreamTermination,
}

/// SSE `data:` payload classified once when the upstream frame is decoded,
/// then shared by telemetry, continuation-ID extraction, and egress model
/// rewriting as validated raw bytes borrowed from the frame.
#[derive(Clone, PartialEq)]
pub enum SseEventPayload {
    /// The frame carries no data lines, such as an SSE comment or heartbeat.
    Empty,
    /// The frame carries the exact `[DONE]` sentinel.
    Done,
    /// The frame carries data lines that are not valid JSON.
    NonJson,
    /// The `event:` name (if present) and the validated JSON data bytes.
    Json(SseJsonData),
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum StreamCompletionPolicy {
    #[default]
    EofAllowed,
    TerminalEventRequired,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum StreamTermination {
    #[default]
    None,
    Completed,
    Failed,
}

impl StreamTermination {
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        !matches!(self, Self::None)
    }
}

impl AdapterEvent {
    #[must_use]
    pub fn new(bytes: Bytes, telemetry: ProtocolEventTelemetry, payload: SseEventPayload) -> Self {
        Self {
            bytes,
            telemetry,
            payload,
            termination: StreamTermination::None,
        }
    }

    #[must_use]
    pub fn with_termination(mut self, termination: StreamTermination) -> Self {
        self.termination = termination;
        self
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
    pub fn payload(&self) -> &SseEventPayload {
        &self.payload
    }

    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        self.termination.is_terminal()
    }

    #[must_use]
    pub const fn termination(&self) -> StreamTermination {
        self.termination
    }

    #[must_use]
    pub fn into_parts(self) -> (Bytes, SseEventPayload) {
        (self.bytes, self.payload)
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
            .field("payload", &self.payload)
            .field("termination", &self.termination)
            .finish()
    }
}

impl fmt::Debug for SseEventPayload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "Empty",
            Self::Done => "Done",
            Self::NonJson => "NonJson",
            Self::Json { .. } => "Json([REDACTED])",
        })
    }
}

#[async_trait]
pub trait ProtocolAdapter: Send + Sync {
    fn dialect(&self) -> ProtocolDialect;

    fn stream_completion_policy(&self, _operation: ProtocolOperation) -> StreamCompletionPolicy {
        StreamCompletionPolicy::EofAllowed
    }

    async fn decode_ingress_request(
        &self,
        request: IngressRequest,
    ) -> Result<DecodedRequest, ProtocolError>;

    fn encode_upstream_request(
        &self,
        operation: ProtocolOperation,
        headers: &HeaderMap,
        payload: &AdapterPayload,
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
        public_model: &str,
    ) -> Result<EgressResponse, ProtocolError>;

    fn encode_egress_event(
        &self,
        event: AdapterEvent,
        public_model: &str,
    ) -> Result<SseFrame, ProtocolError>;

    fn continuation_id_from_response(
        &self,
        _operation: ProtocolOperation,
        _response: &DecodedUpstreamResponse,
    ) -> Result<Option<String>, ProtocolError> {
        Ok(None)
    }

    fn continuation_id_from_event(
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
        request: &DecodedRequest,
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

    fn continuation_state(&self) -> BridgeContinuationState {
        BridgeContinuationState::Stateless
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
            .field("execution_profile", &self.execution_profile)
            .field("client_header_count", &self.client_headers.len())
            .field("body_encoding", &self.body_encoding)
            .field("model", &self.model)
            .field("stream", &self.stream)
            .field("thinking_level", &self.thinking_level)
            .field("affinity", &self.affinity)
            .field("payload", &self.payload)
            .finish()
    }
}

impl fmt::Debug for IngressAffinity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::None => "None",
            Self::Continuation(_) => "Continuation([REDACTED])",
            Self::Session(_) => "Session([REDACTED])",
        })
    }
}

impl fmt::Debug for AdapterPayload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(_) => formatter.write_str("Json([REDACTED])"),
            Self::RawJson(payload) => fmt::Debug::fmt(payload, formatter),
            Self::Multipart(payload) => formatter
                .debug_struct("Multipart")
                .field("part_count", &payload.parts.len())
                .field(
                    "body_bytes",
                    &payload
                        .parts
                        .iter()
                        .map(|part| part.body.len())
                        .sum::<usize>(),
                )
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
            .field(
                "body_bytes",
                &self.body.as_ref().map_or(0, bytes::Bytes::len),
            )
            .finish()
    }
}

impl_redacted_http_debug!(EgressResponse, "EgressResponse");
