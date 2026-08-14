use std::{sync::Arc, time::Duration};

use super::super::{GuardedBody, GuardedBodyParts, PrecommitBudget};
use super::core::generation_permit;
use crate::{
    affinity::{
        AffinityRegistry, AffinityTarget, ContinuationBindingCommitter, ContinuationLookup,
    },
    request_telemetry::AttemptRecorder,
};
use any2api_domain::{ErrorClass, ModelRouteId, ProtocolDialect, ProtocolOperation, RouteTargetId};
use any2api_protocol::{
    OpenAiResponsesAdapter,
    api::{DecodedRequest, IngressRequest, ProtocolRegistry},
};
use any2api_transport::api::BoxByteStream;
use bytes::Bytes;
use futures_util::stream;
use http::{HeaderMap, Method, Uri};

const CONTINUATION_TTL: Duration = Duration::from_secs(60);

#[tokio::test]
async fn failed_handoff_rolls_back_only_its_provisional_stateless_binding() {
    let registry = AffinityRegistry::new();
    let protocols = protocols();
    let response_id = "resp_handoff_failure";
    let (body, _) = stateless_body(&protocols, Arc::clone(&registry), stateless_upstream()).await;
    let mut body = body
        .prime_attempt()
        .await
        .expect("stateless identity is provisionally bound");

    assert!(matches!(
        registry.resolve_continuation(response_id, CONTINUATION_TTL, |_| true),
        ContinuationLookup::Pending
    ));

    body.fail_before_handoff(ErrorClass::Internal, "session handoff failed");
    assert!(matches!(
        registry.resolve_continuation(response_id, CONTINUATION_TTL, |_| true),
        ContinuationLookup::Missing
    ));

    let (body, target) =
        stateless_body(&protocols, Arc::clone(&registry), stateless_upstream()).await;
    let mut body = body
        .prime_attempt()
        .await
        .expect("second stateless identity is provisionally bound");

    assert_eq!(registry.clear_all(), 1);
    let mut replacement = registry
        .begin_pending_continuation(response_id, target, CONTINUATION_TTL)
        .expect("newer provisional binding");

    body.fail_before_handoff(ErrorClass::Internal, "session handoff failed");
    assert!(matches!(
        registry.resolve_continuation(response_id, CONTINUATION_TTL, |_| true),
        ContinuationLookup::Pending
    ));

    replacement
        .ready(None, None)
        .expect("newer provisional binding commits");
    drop(body);
    let ready = registry.resolve_continuation(response_id, CONTINUATION_TTL, |_| true);
    let ContinuationLookup::Ready(ready) = ready else {
        panic!("newer committed binding must remain ready: {ready:?}");
    };
    let (_, state) = ready.into_parts();
    assert!(state.is_none());
}

fn stateless_upstream() -> BoxByteStream {
    Box::pin(stream::iter([
        Ok(Bytes::from_static(
            b"event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_handoff_failure\",\"model\":\"upstream\"}}\n\n",
        )),
        Ok(Bytes::from_static(
            b"event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{}}\n\n",
        )),
    ]))
}

async fn stateless_body(
    protocols: &ProtocolRegistry,
    registry: Arc<AffinityRegistry>,
    upstream: BoxByteStream,
) -> (GuardedBody, AffinityTarget) {
    let (_, permit) = generation_permit();
    let target = AffinityTarget::new(
        ModelRouteId::new(),
        RouteTargetId::new(),
        permit.credential_id(),
        "upstream",
        ProtocolDialect::OpenAiResponses,
        ProtocolDialect::OpenAiResponses,
    );
    let continuation_binding = ContinuationBindingCommitter::new(
        ProtocolOperation::Responses,
        registry,
        target.clone(),
        CONTINUATION_TTL,
    );
    let request = decoded(protocols).await;
    let mut exchange = protocols
        .exchange(
            ProtocolDialect::OpenAiResponses,
            ProtocolDialect::OpenAiResponses,
            ProtocolOperation::Responses,
        )
        .expect("direct Responses exchange");
    exchange
        .prepare_request(&request, "upstream", None)
        .expect("direct Responses request");
    let body = GuardedBody::new(
        upstream,
        exchange,
        "public",
        GuardedBodyParts {
            permit,
            health: None,
            continuation_binding,
            attempt_recorder: AttemptRecorder::disabled(),
            quota_activity: None,
            status_code: 200,
            precommit_budget: PrecommitBudget::new(256 * 1024, Duration::from_secs(5)),
            postcommit_idle_timeout: Duration::from_secs(60),
        },
    );
    (body, target)
}

async fn decoded(protocols: &ProtocolRegistry) -> DecodedRequest {
    protocols
        .get(ProtocolDialect::OpenAiResponses)
        .expect("Responses adapter")
        .decode_ingress_request(IngressRequest {
            method: Method::POST,
            uri: Uri::from_static("/v1/responses"),
            headers: HeaderMap::new(),
            body: Bytes::from_static(b"{\"model\":\"public\",\"input\":\"start\",\"stream\":true}"),
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
}
