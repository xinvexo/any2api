use std::sync::Arc;

use any2api_domain::{ProtocolDialect, ProtocolOperation};
use bytes::Bytes;
use http::{HeaderMap, Method, StatusCode, Uri};
use serde_json::{Value, json};

use super::ImagesToChatCompletionsBridge;
use crate::{
    OpenAiChatCompletionsAdapter, OpenAiImagesAdapter, ProtocolError, ProtocolRegistry,
    api::{IngressRequest, ProtocolAdapter, UpstreamResponse},
};

#[tokio::test]
async fn converts_image_generation_request_to_chat_messages() {
    let decoded = decoded_request(json!({
        "model":"public-image",
        "prompt":"Draw a red circle",
        "stream":false,
        "partial_images":0,
        "response_format":"url",
        "n":2,
        "size":"1024x1024",
        "quality":"low",
        "background":"opaque",
        "moderation":"auto",
        "output_format":"png",
        "output_compression":90,
        "style":"natural",
        "user":"user-1"
    }))
    .await;
    let mut exchange = registry()
        .exchange(
            ProtocolDialect::OpenAiImages,
            ProtocolDialect::OpenAiChatCompletions,
            ProtocolOperation::ImagesGenerations,
        )
        .expect("Images bridge exchange");

    let prepared = exchange
        .prepare_request(&decoded, "upstream-image", None)
        .expect("converted request");
    let body: Value = serde_json::from_slice(&prepared.request.body).expect("Chat JSON");

    assert_eq!(
        prepared.upstream_operation,
        ProtocolOperation::ChatCompletions
    );
    assert_eq!(body["model"], "upstream-image");
    assert_eq!(
        body["messages"],
        json!([{
            "role":"user",
            "content":"Draw a red circle"
        }])
    );
    assert_eq!(body["stream"], false);
    for field in [
        "n",
        "size",
        "quality",
        "background",
        "moderation",
        "output_format",
        "output_compression",
        "style",
        "user",
    ] {
        assert_eq!(
            body[field],
            decoded.payload.materialize_json().unwrap()[field]
        );
    }
    assert!(body.get("prompt").is_none());
    assert!(body.get("partial_images").is_none());
    assert!(body.get("response_format").is_none());
}

#[tokio::test]
async fn rejects_unrepresentable_image_requests_before_upstream_encoding() {
    for body in [
        json!({"model":"image","prompt":"draw","stream":true}),
        json!({"model":"image","prompt":"draw","partial_images":1}),
        json!({"model":"image","prompt":"draw","response_format":"b64_json"}),
        json!({"model":"image","prompt":"draw","future_field":true}),
        json!({"model":"image","prompt":"draw","n":0}),
        json!({"model":"image","prompt":"   "}),
    ] {
        let decoded = decoded_request(body).await;
        let mut exchange = bridge_exchange();
        assert!(matches!(
            exchange.prepare_request(&decoded, "upstream-image", None),
            Err(ProtocolError::InvalidPayload(_))
        ));
    }

    let protocols = registry();
    assert!(!protocols.supports_operation(
        ProtocolDialect::OpenAiImages,
        ProtocolDialect::OpenAiChatCompletions,
        ProtocolOperation::ImagesEdits,
    ));
}

#[tokio::test]
async fn converts_markdown_image_choices_and_chat_usage_to_images_response() {
    let mut exchange = prepared_exchange(2).await;
    let decoded = exchange
        .decode_upstream_response(upstream_response(json!({
            "id":"chatcmpl_image",
            "object":"chat.completion",
            "created":1785916638_u64,
            "model":"upstream-image",
            "choices":[
                {
                    "index":1,
                    "finish_reason":"stop",
                    "message":{
                        "role":"assistant",
                        "content":"![second](https://images.example/second.png?token=two)"
                    }
                },
                {
                    "index":0,
                    "finish_reason":"stop",
                    "message":{
                        "role":"assistant",
                        "content":"https://images.example/first.png?token=one"
                    }
                }
            ],
            "usage":{
                "prompt_tokens":11,
                "completion_tokens":22,
                "total_tokens":33
            }
        })))
        .expect("converted response");
    let response = exchange
        .encode_egress_response(decoded, "public-image")
        .expect("Images response");
    let body: Value = serde_json::from_slice(&response.body).expect("Images JSON");

    assert_eq!(body["created"], 1_785_916_638_u64);
    assert_eq!(
        body["data"],
        json!([
            {"url":"https://images.example/first.png?token=one"},
            {"url":"https://images.example/second.png?token=two"}
        ])
    );
    assert_eq!(
        body["usage"],
        json!({"input_tokens":11,"output_tokens":22,"total_tokens":33})
    );
    assert!(body.get("model").is_none());
}

#[tokio::test]
async fn rejects_invalid_chat_image_success_responses_without_partial_output() {
    for body in [
        json!({
            "created":1,
            "choices":[{"index":0,"finish_reason":"length","message":{
                "role":"assistant","content":"![image](https://images.example/a.png)"
            }}]
        }),
        json!({
            "created":1,
            "choices":[{"index":0,"finish_reason":"stop","message":{
                "role":"assistant","content":"Here is some ordinary text"
            }}]
        }),
        json!({
            "created":1,
            "choices":[{"index":0,"finish_reason":"stop","message":{
                "role":"assistant","content":"![image](data:image/png;base64,AAAA)"
            }}]
        }),
        json!({
            "created":1,
            "choices":[{"index":0,"finish_reason":"stop","message":{
                "role":"assistant","content":"before ![image](https://images.example/a.png)"
            }}]
        }),
    ] {
        let mut exchange = prepared_exchange(1).await;
        assert!(matches!(
            exchange.decode_upstream_response(upstream_response(body)),
            Err(ProtocolError::InvalidPayload(_))
        ));
    }

    let mut duplicate = prepared_exchange(2).await;
    assert!(matches!(
        duplicate.decode_upstream_response(upstream_response(json!({
            "created":1,
            "choices":[
                {"index":0,"finish_reason":"stop","message":{
                    "role":"assistant","content":"https://images.example/a.png"
                }},
                {"index":0,"finish_reason":"stop","message":{
                    "role":"assistant","content":"https://images.example/b.png"
                }}
            ]
        }))),
        Err(ProtocolError::InvalidPayload(_))
    ));
}

async fn prepared_exchange(n: usize) -> crate::api::ProtocolExchange {
    let mut exchange = bridge_exchange();
    let decoded = decoded_request(json!({
        "model":"public-image",
        "prompt":"draw",
        "n":n
    }))
    .await;
    exchange
        .prepare_request(&decoded, "upstream-image", None)
        .expect("prepare Images bridge");
    exchange
}

fn bridge_exchange() -> crate::api::ProtocolExchange {
    registry()
        .exchange(
            ProtocolDialect::OpenAiImages,
            ProtocolDialect::OpenAiChatCompletions,
            ProtocolOperation::ImagesGenerations,
        )
        .expect("Images bridge exchange")
}

async fn decoded_request(body: Value) -> crate::api::DecodedRequest {
    OpenAiImagesAdapter::new()
        .decode_ingress_request(IngressRequest {
            method: Method::POST,
            uri: Uri::from_static("/v1/images/generations"),
            headers: HeaderMap::new(),
            body: Bytes::from(serde_json::to_vec(&body).expect("request JSON")),
            operation: ProtocolOperation::ImagesGenerations,
        })
        .await
        .expect("Images request")
}

fn upstream_response(body: Value) -> UpstreamResponse {
    UpstreamResponse {
        status: StatusCode::OK,
        headers: HeaderMap::new(),
        body: Bytes::from(serde_json::to_vec(&body).expect("response JSON")),
    }
}

fn registry() -> ProtocolRegistry {
    let mut registry = ProtocolRegistry::new();
    registry
        .register(Arc::new(OpenAiImagesAdapter::new()))
        .expect("Images adapter");
    registry
        .register(Arc::new(OpenAiChatCompletionsAdapter::new()))
        .expect("Chat adapter");
    registry
        .register_bridge(Arc::new(ImagesToChatCompletionsBridge::new()))
        .expect("Images bridge");
    registry
}
