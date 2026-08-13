use std::{sync::Arc, time::Duration};

use any2api_domain::{ModelRouteId, ProtocolDialect, ProtocolOperation, RouteTargetId};
use any2api_protocol::{
    OpenAiChatCompletionsAdapter, OpenAiResponsesAdapter, ResponsesToChatCompletionsBridge,
    api::{DecodedRequest, IngressRequest, MAX_BRIDGE_CONTINUATION_STATE_BYTES, ProtocolRegistry},
};
use any2api_transport::api::BoxByteStream;
use bytes::Bytes;
use futures_util::{StreamExt, future, stream};
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
async fn bridge_prime_drains_suppressed_frames_before_waiting_for_more_network() {
    let registry = AffinityRegistry::new();
    let protocols = protocols();
    let upstream: BoxByteStream = Box::pin(
        stream::iter([Ok(Bytes::from_static(
            b": keep-alive\n\n: still-alive\n\ndata: {\"id\":\"chatcmpl_buffered\",\"model\":\"upstream\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"Hello\"}}]}\n\n",
        ))])
        .chain(stream::pending()),
    );

    let guarded = tokio::time::timeout(
        Duration::from_millis(100),
        bridged_body(&protocols, registry, upstream).await.prime(),
    )
    .await
    .expect("buffered valid frame must not wait for another upstream chunk")
    .expect("bridge first event");

    assert!(!pending_response_id(&guarded).is_empty());
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

#[tokio::test]
async fn aborting_a_task_that_owns_a_primed_body_drops_the_pending_lease() {
    let registry = AffinityRegistry::new();
    let task_registry = Arc::clone(&registry);
    let (identity_sender, identity_receiver) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let protocols = protocols();
        let upstream: BoxByteStream = Box::pin(
            stream::iter([Ok(Bytes::from_static(
                b"data: {\"id\":\"chatcmpl_cancel\",\"model\":\"upstream\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"partial\"}}]}\n\n",
            ))])
            .chain(stream::pending()),
        );
        let guarded = bridged_body(&protocols, task_registry, upstream)
            .await
            .prime()
            .await
            .expect("bridge first event");
        identity_sender
            .send(pending_response_id(&guarded))
            .expect("test receives pending identity");
        future::pending::<()>().await;
        drop(guarded);
    });
    let response_id = identity_receiver.await.expect("pending identity");

    assert!(matches!(
        registry.resolve_continuation(&response_id, CONTINUATION_TTL, |_| true),
        ContinuationLookup::Pending
    ));
    task.abort();
    assert!(
        task.await
            .expect_err("owner task is cancelled")
            .is_cancelled()
    );

    assert!(matches!(
        registry.resolve_continuation(&response_id, CONTINUATION_TTL, |_| true),
        ContinuationLookup::Missing
    ));
    assert_eq!(registry.continuation_bytes_for_test(), 0);
}

#[tokio::test]
async fn failed_or_invalid_bridge_aborts_the_pending_continuation() {
    let cases: [(&str, &'static [u8], bool); 2] = [
        (
            "failure event",
            b"data: {\"error\":{\"message\":\"provider is overloaded\",\"type\":\"server_error\",\"param\":null,\"code\":\"overloaded\"}}\n\n",
            false,
        ),
        (
            "invalid tool identity",
            b"data: {\"id\":\"chatcmpl_invalid\",\"model\":\"upstream\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"type\":\"function\",\"function\":{\"name\":\"broken\",\"arguments\":\"{}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\n",
            true,
        ),
    ];

    for (name, terminal, expect_body_error) in cases {
        let registry = AffinityRegistry::new();
        let protocols = protocols();
        let upstream: BoxByteStream = Box::pin(stream::iter([
            Ok(Bytes::from_static(
                b"data: {\"id\":\"chatcmpl_pending\",\"model\":\"upstream\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"partial\"}}]}\n\n",
            )),
            Ok(Bytes::from_static(terminal)),
        ]));
        let guarded = bridged_body(&protocols, Arc::clone(&registry), upstream)
            .await
            .prime()
            .await
            .unwrap_or_else(|error| panic!("{name} must commit after partial content: {error:?}"));
        let response_id = pending_response_id(&guarded);
        let mut body = guarded.into_stream();
        let mut body_error = false;
        while let Some(frame) = body.next().await {
            body_error |= frame.is_err();
        }

        assert_eq!(body_error, expect_body_error, "{name}");
        assert!(matches!(
            registry.resolve_continuation(&response_id, CONTINUATION_TTL, |_| true),
            ContinuationLookup::Missing
        ));
        assert_eq!(registry.continuation_bytes_for_test(), 0, "{name}");
    }
}

#[tokio::test(start_paused = true)]
async fn pending_bridge_idle_timeout_drops_the_lease_and_full_reservation() {
    let registry = AffinityRegistry::new();
    let protocols = protocols();
    let upstream: BoxByteStream = Box::pin(
        stream::iter([Ok(Bytes::from_static(
            b"data: {\"id\":\"chatcmpl_idle\",\"model\":\"upstream\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"partial\"}}]}\n\n",
        ))])
        .chain(stream::pending()),
    );
    let guarded = bridged_body_with_idle_timeout(
        &protocols,
        Arc::clone(&registry),
        upstream,
        Duration::from_millis(25),
    )
    .await
    .prime()
    .await
    .expect("bridge first event");
    let response_id = pending_response_id(&guarded);
    let mut body = guarded.into_stream();

    let mut delivered = 0;
    let error = loop {
        match body.next().await.expect("idle timeout body item") {
            Ok(_) => delivered += 1,
            Err(error) => break error,
        }
    };
    assert!(delivered > 0);
    assert!(error.to_string().contains("idle after commit"));
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
    bridged_body_with_idle_timeout(protocols, registry, upstream, Duration::from_secs(60)).await
}

async fn bridged_body_with_idle_timeout(
    protocols: &ProtocolRegistry,
    registry: Arc<AffinityRegistry>,
    upstream: BoxByteStream,
    postcommit_idle_timeout: Duration,
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
            quota_activity: None,
            cache_locality: None,
            status_code: 200,
            precommit_budget: PrecommitBudget::new(256 * 1024, Duration::from_secs(5)),
            postcommit_idle_timeout,
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
