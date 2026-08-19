use std::fmt;

use any2api_domain::{RequestSpeedTier, TokenUsage};
use bytes::Bytes;
use http::{HeaderMap, StatusCode};
use serde_json::Value;

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
    /// Direct exchanges retain validated wire bytes without materializing a
    /// full JSON tree; bridges receive and return structured JSON instead.
    pub payload: DecodedResponsePayload,
    pub telemetry: ProtocolResponseTelemetry,
}

#[derive(Clone)]
pub enum DecodedResponsePayload {
    RawJson(Bytes),
    StructuredJson(Value),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProtocolResponseTelemetry {
    pub token_usage: TokenUsage,
    pub effective_speed_tier: Option<RequestSpeedTier>,
}

#[derive(Clone)]
pub struct EgressResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
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

impl_redacted_http_debug!(UpstreamResponse, "UpstreamResponse");

impl fmt::Debug for DecodedUpstreamResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DecodedUpstreamResponse")
            .field("status", &self.status)
            .field("header_count", &self.headers.len())
            .field(
                "body_bytes",
                &match &self.payload {
                    DecodedResponsePayload::RawJson(body) => body.len(),
                    DecodedResponsePayload::StructuredJson(_) => 0,
                },
            )
            .finish()
    }
}

impl_redacted_http_debug!(EgressResponse, "EgressResponse");
