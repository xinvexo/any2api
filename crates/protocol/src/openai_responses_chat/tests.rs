use std::sync::Arc;

use any2api_domain::{ProtocolDialect, ProtocolOperation};
use bytes::Bytes;
use http::{HeaderMap, Method, StatusCode, Uri};
use serde_json::Value;

use super::ResponsesToChatCompletionsBridge;
use crate::{
    OpenAiChatCompletionsAdapter, OpenAiResponsesAdapter, ProtocolRegistry,
    api::{IngressRequest, SseFrame, UpstreamResponse},
};

mod buffered;
mod streaming;
mod validation;

pub(super) fn registry() -> ProtocolRegistry {
    let mut registry = ProtocolRegistry::new();
    registry
        .register(Arc::new(OpenAiResponsesAdapter::new()))
        .expect("Responses adapter");
    registry
        .register(Arc::new(OpenAiChatCompletionsAdapter::new()))
        .expect("Chat adapter");
    registry
        .register_bridge(Arc::new(ResponsesToChatCompletionsBridge::new()))
        .expect("Responses to Chat bridge");
    registry
}

pub(super) async fn decoded(
    registry: &ProtocolRegistry,
    operation: ProtocolOperation,
    body: Value,
) -> crate::api::DecodedRequest {
    registry
        .get(operation.dialect())
        .expect("ingress adapter")
        .decode_ingress_request(IngressRequest {
            method: Method::POST,
            uri: Uri::from_static("/v1/test"),
            headers: HeaderMap::new(),
            body: Bytes::from(serde_json::to_vec(&body).expect("request JSON")),
            operation,
        })
        .await
        .expect("decoded request")
}

pub(super) fn bridged_exchange(
    registry: &ProtocolRegistry,
    operation: ProtocolOperation,
) -> crate::api::ProtocolExchange {
    registry
        .exchange(
            ProtocolDialect::OpenAiResponses,
            ProtocolDialect::OpenAiChatCompletions,
            operation,
        )
        .expect("protocol exchange")
}

pub(super) fn upstream_response(body: Value) -> UpstreamResponse {
    UpstreamResponse {
        status: StatusCode::OK,
        headers: HeaderMap::new(),
        body: Bytes::from(serde_json::to_vec(&body).expect("response JSON")),
    }
}

pub(super) fn chat_frame(value: Value) -> SseFrame {
    SseFrame(Bytes::from(format!("data: {value}\n\n")))
}
