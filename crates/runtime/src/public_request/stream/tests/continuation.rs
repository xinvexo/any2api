use std::{sync::Arc, time::Duration};

use any2api_domain::{ModelRouteId, ProtocolDialect, ProtocolOperation, RouteTargetId};
use any2api_protocol::{
    OpenAiChatCompletionsAdapter, OpenAiResponsesAdapter, ProtocolRegistry,
    ResponsesToChatCompletionsBridge,
    api::{DecodedRequest, IngressRequest, MAX_BRIDGE_CONTINUATION_STATE_BYTES},
};
use any2api_transport::api::BoxByteStream;
use bytes::Bytes;
use futures_util::{StreamExt, stream};
use http::{HeaderMap, Method, Uri};
use serde_json::{Value, json};

use super::super::{GuardedBody, GuardedBodyParts, PrecommitBudget};
use super::core::generation_permit;
use crate::{
    affinity::{
        AffinityRegistry, AffinityTarget, ContinuationBindingCommitter, ContinuationLookup,
    },
    request_telemetry::AttemptRecorder,
};

const CONTINUATION_TTL: Duration = Duration::from_secs(60);

#[tokio::test]
async fn bridged_stream_commits_pending_then_ready_and_restores_history() {
    let registry = AffinityRegistry::new();
    let protocols = protocols();
    let upstream: BoxByteStream = Box::pin(stream::iter([
        Ok(Bytes::from_static(
            b"data: {\"id\":\"chatcmpl_runtime\",\"model\":\"upstream\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"Hello\"}}]}\n\n",
        )),
        Ok(Bytes::from_static(
            b"data: {\"id\":\"chatcmpl_runtime\",\"model\":\"upstream\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n",
        )),
    ]));
    let guarded = bridged_body(&protocols, Arc::clone(&registry), upstream)
        .await
        .prime()
        .await
        .expect("bridge first event");
    let response_id = pending_response_id(&guarded);

    assert!(matches!(
        registry.resolve_continuation(&response_id, CONTINUATION_TTL, |_| true),
        ContinuationLookup::Pending
    ));
    assert_eq!(
        registry.continuation_bytes_for_test(),
        MAX_BRIDGE_CONTINUATION_STATE_BYTES
    );

    let mut body = guarded.into_stream();
    while let Some(frame) = body.next().await {
        frame.expect("valid bridged frame");
    }
    let resolved = match registry.resolve_continuation(&response_id, CONTINUATION_TTL, |_| true) {
        ContinuationLookup::Ready(resolved) => resolved,
        other => panic!("completed bridge must be Ready: {other:?}"),
    };
    let (_, continuation) = resolved.into_parts();
    let continuation = continuation.expect("bridged history state");
    assert_eq!(
        registry.continuation_bytes_for_test(),
        continuation.serialized_bytes()
    );
    assert!(continuation.serialized_bytes() < MAX_BRIDGE_CONTINUATION_STATE_BYTES);

    let follow_up = decoded(
        &protocols,
        json!({
            "model":"public",
            "previous_response_id":response_id,
            "input":"continue"
        }),
    )
    .await;
    let mut exchange = protocols
        .exchange(
            ProtocolDialect::OpenAiResponses,
            ProtocolDialect::OpenAiChatCompletions,
            ProtocolOperation::Responses,
        )
        .expect("bridge exchange");
    let prepared = exchange
        .prepare_request(&follow_up, "upstream", Some(continuation))
        .expect("restore bridge history");
    let body: Value = serde_json::from_slice(&prepared.request.body).expect("upstream JSON");
    assert_eq!(body["messages"][0]["content"], "start");
    assert_eq!(body["messages"][1]["role"], "assistant");
    assert_eq!(body["messages"][1]["content"], "Hello");
    assert_eq!(body["messages"][2]["content"], "continue");
    assert_eq!(registry.sweep_expired(Duration::ZERO), 1);
    assert_eq!(registry.continuation_bytes_for_test(), 0);
}

#[tokio::test]
async fn dropping_pending_bridged_body_removes_binding_and_full_reservation() {
    let registry = AffinityRegistry::new();
    let protocols = protocols();
    let upstream: BoxByteStream = Box::pin(
        stream::iter([Ok(Bytes::from_static(
            b"data: {\"id\":\"chatcmpl_drop\",\"model\":\"upstream\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"partial\"}}]}\n\n",
        ))])
        .chain(stream::pending()),
    );
    let guarded = bridged_body(&protocols, Arc::clone(&registry), upstream)
        .await
        .prime()
        .await
        .expect("bridge first event");
    let response_id = pending_response_id(&guarded);
    assert!(matches!(
        registry.resolve_continuation(&response_id, CONTINUATION_TTL, |_| true),
        ContinuationLookup::Pending
    ));
    assert_eq!(
        registry.continuation_bytes_for_test(),
        MAX_BRIDGE_CONTINUATION_STATE_BYTES
    );

    drop(guarded);

    assert!(matches!(
        registry.resolve_continuation(&response_id, CONTINUATION_TTL, |_| true),
        ContinuationLookup::Missing
    ));
    assert_eq!(registry.continuation_bytes_for_test(), 0);
}

async fn bridged_body(
    protocols: &ProtocolRegistry,
    registry: Arc<AffinityRegistry>,
    upstream: BoxByteStream,
) -> GuardedBody {
    let (_, permit) = generation_permit();
    let target = AffinityTarget::new(
        ModelRouteId::new(),
        RouteTargetId::new(),
        permit.credential_id(),
        "upstream",
        ProtocolDialect::OpenAiResponses,
        ProtocolDialect::OpenAiChatCompletions,
    );
    let continuation_binding = ContinuationBindingCommitter::new(
        ProtocolOperation::Responses,
        registry,
        target,
        CONTINUATION_TTL,
    );
    let request = decoded(
        protocols,
        json!({"model":"public","input":"start","stream":true}),
    )
    .await;
    let mut exchange = protocols
        .exchange(
            ProtocolDialect::OpenAiResponses,
            ProtocolDialect::OpenAiChatCompletions,
            ProtocolOperation::Responses,
        )
        .expect("bridge exchange");
    exchange
        .prepare_request(&request, "upstream", None)
        .expect("bridge request");
    GuardedBody::new(
        upstream,
        exchange,
        "public",
        GuardedBodyParts {
            permit,
            health: None,
            continuation_binding,
            attempt_recorder: AttemptRecorder::disabled(),
            status_code: 200,
            precommit_budget: PrecommitBudget::new(256 * 1024, Duration::from_secs(5)),
            postcommit_idle_timeout: Duration::from_secs(60),
        },
    )
}

async fn decoded(protocols: &ProtocolRegistry, body: Value) -> DecodedRequest {
    protocols
        .get(ProtocolDialect::OpenAiResponses)
        .expect("Responses adapter")
        .decode_ingress_request(IngressRequest {
            method: Method::POST,
            uri: Uri::from_static("/v1/responses"),
            headers: HeaderMap::new(),
            body: Bytes::from(serde_json::to_vec(&body).expect("request JSON")),
            operation: ProtocolOperation::Responses,
        })
        .await
        .expect("decoded request")
}

fn protocols() -> ProtocolRegistry {
    let mut protocols = ProtocolRegistry::new();
    protocols
        .register(Arc::new(OpenAiResponsesAdapter::new()))
        .expect("Responses adapter");
    protocols
        .register(Arc::new(OpenAiChatCompletionsAdapter::new()))
        .expect("Chat adapter");
    protocols
        .register_bridge(Arc::new(ResponsesToChatCompletionsBridge::new()))
        .expect("Responses to Chat bridge");
    protocols
}

fn pending_response_id(body: &GuardedBody) -> String {
    let frame = body.pending.front().expect("response.created frame");
    let text = std::str::from_utf8(&frame.bytes).expect("UTF-8 SSE");
    let data = text
        .lines()
        .find_map(|line| line.strip_prefix("data: "))
        .expect("SSE data line");
    let value: Value = serde_json::from_str(data).expect("SSE data JSON");
    value["response"]["id"]
        .as_str()
        .expect("response.created ID")
        .to_owned()
}
