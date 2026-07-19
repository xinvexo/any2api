use std::fmt;

use any2api_domain::{ProtocolDialect, ProtocolOperation, PublicError};
use bytes::Bytes;
use http::{HeaderMap, Method, StatusCode, Uri};

pub use crate::{ProtocolError, ProtocolRegistry};

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
    pub payload: AdapterPayload,
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
}

#[derive(Clone)]
pub struct EgressResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SseFrame(pub Bytes);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterEvent(pub Bytes);

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

    fn encode_egress_event(&self, event: AdapterEvent) -> Result<SseFrame, ProtocolError>;

    fn error_response(&self, error: &PublicError) -> EgressResponse;
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
            .field("payload", &self.payload)
            .finish()
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
