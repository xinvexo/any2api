use any2api_domain::{ProtocolDialect, PublicError};
use bytes::Bytes;
use http::{HeaderMap, Method, StatusCode, Uri};

pub use crate::{ProtocolError, ProtocolRegistry};

#[derive(Clone, Debug)]
pub struct IngressRequest {
    pub method: Method,
    pub uri: Uri,
    pub headers: HeaderMap,
    pub body: Bytes,
}

#[derive(Clone, Debug)]
pub struct DecodedRequest {
    pub dialect: ProtocolDialect,
    pub model: Option<String>,
    pub stream: bool,
    pub payload: AdapterPayload,
}

#[derive(Clone, Debug)]
pub enum AdapterPayload {
    RawJson(Bytes),
}

#[derive(Clone, Debug)]
pub struct EncodedUpstreamRequest {
    pub method: Method,
    pub uri: Uri,
    pub headers: HeaderMap,
    pub body: Bytes,
}

#[derive(Clone, Debug)]
pub struct UpstreamResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
}

#[derive(Clone, Debug)]
pub struct DecodedUpstreamResponse {
    pub payload: AdapterPayload,
}

#[derive(Clone, Debug)]
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
        payload: AdapterPayload,
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
